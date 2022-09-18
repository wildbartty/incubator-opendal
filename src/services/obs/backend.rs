// Copyright 2022 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::collections::HashMap;
use std::fmt::Debug;
use std::fmt::Write;
use std::io::Result;
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use http::header::CONTENT_LENGTH;
use http::Request;
use http::Response;
use http::StatusCode;
use http::Uri;
use log::debug;
use log::info;
use reqsign::services::huaweicloud::obs::Signer;

use super::error::parse_error;
use crate::accessor::AccessorCapability;
use crate::error::other;
use crate::error::BackendError;
use crate::error::ObjectError;
use crate::http_util::new_request_build_error;
use crate::http_util::new_request_send_error;
use crate::http_util::new_request_sign_error;
use crate::http_util::new_response_consume_error;
use crate::http_util::parse_content_length;
use crate::http_util::parse_error_response;
use crate::http_util::parse_etag;
use crate::http_util::parse_last_modified;
use crate::http_util::percent_encode_path;
use crate::http_util::AsyncBody;
use crate::http_util::HttpClient;
use crate::ops::BytesRange;
use crate::ops::OpCreate;
use crate::ops::OpDelete;
use crate::ops::OpList;
use crate::ops::OpRead;
use crate::ops::OpStat;
use crate::ops::OpWrite;
use crate::ops::Operation;
use crate::path::build_abs_path;
use crate::path::normalize_root;
use crate::services::obs::dir_stream::DirStream;
use crate::Accessor;
use crate::AccessorMetadata;
use crate::BytesReader;
use crate::DirStreamer;
use crate::ObjectMetadata;
use crate::ObjectMode;
use crate::Scheme;

/// Builder for Huaweicloud OBS services
#[derive(Default, Clone)]
pub struct Builder {
    root: Option<String>,
    endpoint: Option<String>,
    access_key_id: Option<String>,
    secret_access_key: Option<String>,
    bucket: Option<String>,
}

impl Debug for Builder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Builder")
            .field("root", &self.root)
            .field("endpoint", &self.endpoint)
            .field("access_key_id", &"<redacted>")
            .field("secret_access_key", &"<redacted>")
            .field("bucket", &self.bucket)
            .finish()
    }
}

impl Builder {
    /// Set root of this backend.
    ///
    /// All operations will happen under this root.
    pub fn root(&mut self, root: &str) -> &mut Self {
        if !root.is_empty() {
            self.root = Some(root.to_string())
        }

        self
    }

    /// Set endpoint of this backend.
    ///
    /// Both huaweicloud default domain and user domain endpoints are allowed.
    /// Please DO NOT add the bucket name to the endpoint.
    ///
    /// - `https://obs.cn-north-4.myhuaweicloud.com`
    /// - `obs.cn-north-4.myhuaweicloud.com` (https by default)
    /// - `https://custom.obs.com` (port should not be set)
    pub fn endpoint(&mut self, endpoint: &str) -> &mut Self {
        if !endpoint.is_empty() {
            self.endpoint = Some(endpoint.trim_end_matches('/').to_string());
        }

        self
    }

    /// Set access_key_id of this backend.
    /// - If it is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub fn access_key_id(&mut self, access_key_id: &str) -> &mut Self {
        if !access_key_id.is_empty() {
            self.access_key_id = Some(access_key_id.to_string());
        }

        self
    }

    /// Set secret_access_key of this backend.
    /// - If it is set, we will take user's input first.
    /// - If not, we will try to load it from environment.
    pub fn secret_access_key(&mut self, secret_access_key: &str) -> &mut Self {
        if !secret_access_key.is_empty() {
            self.secret_access_key = Some(secret_access_key.to_string());
        }

        self
    }

    /// Set bucket of this backend.
    /// The param is required.
    pub fn bucket(&mut self, bucket: &str) -> &mut Self {
        if !bucket.is_empty() {
            self.bucket = Some(bucket.to_string());
        }

        self
    }

    /// Consume builder to build an OBS backend.
    pub fn build(&mut self) -> Result<Backend> {
        info!("backend build started: {:?}", &self);

        let root = normalize_root(&self.root.take().unwrap_or_default());
        info!("backend use root {}", root);

        let bucket = match &self.bucket {
            Some(bucket) => Ok(bucket.to_string()),
            None => Err(other(BackendError::new(
                HashMap::from([("bucket".to_string(), "".to_string())]),
                anyhow!("bucket is empty"),
            ))),
        }?;
        debug!("backend use bucket {}", &bucket);

        let uri = match &self.endpoint {
            Some(endpoint) => endpoint.parse::<Uri>().map_err(|_| {
                other(BackendError::new(
                    HashMap::from([("endpoint".to_string(), "".to_string())]),
                    anyhow!("endpoint is invalid"),
                ))
            }),
            None => Err(other(BackendError::new(
                HashMap::from([("endpoint".to_string(), "".to_string())]),
                anyhow!("endpoint is empty"),
            ))),
        }?;

        let scheme = match uri.scheme_str() {
            Some(scheme) => scheme.to_string(),
            None => "https".to_string(),
        };

        let (endpoint, is_obs_default) = {
            let host = uri.host().unwrap_or_default().to_string();
            if host.starts_with("obs.") && host.ends_with(".myhuaweicloud.com") {
                (format!("{}.{}", bucket, host), true)
            } else {
                (host, false)
            }
        };

        debug!("backend use endpoint {}", &endpoint);

        let context = HashMap::from([
            ("bucket".to_string(), bucket.to_string()),
            ("endpoint".to_string(), endpoint.to_string()),
        ]);

        let client = HttpClient::new();

        let mut signer_builder = Signer::builder();
        if let (Some(access_key_id), Some(secret_access_key)) =
            (&self.access_key_id, &self.secret_access_key)
        {
            signer_builder
                .access_key(access_key_id)
                .secret_key(secret_access_key);
        }

        // Set the bucket name in CanonicalizedResource.
        // 1. If the bucket is bound to a user domain name, use the user domain name as the bucket name,
        // for example, `/obs.ccc.com/object`. `obs.ccc.com` is the user domain name bound to the bucket.
        // 2. If you do not access OBS using a user domain name, this field is in the format of `/bucket/object`.
        //
        // Please refer to this doc for more details:
        // https://support.huaweicloud.com/intl/en-us/api-obs/obs_04_0010.html
        if is_obs_default {
            signer_builder.bucket(&bucket);
        } else {
            signer_builder.bucket(&endpoint);
        }

        let signer = signer_builder
            .build()
            .map_err(|e| other(BackendError::new(context, e)))?;

        info!("backend build finished: {:?}", &self);
        Ok(Backend {
            client,
            root,
            endpoint: format!("{}://{}", &scheme, &endpoint),
            signer: Arc::new(signer),
            bucket,
        })
    }
}

/// Backend for Huaweicloud OBS services.
#[derive(Debug, Clone)]
pub struct Backend {
    client: HttpClient,
    root: String,
    endpoint: String,
    signer: Arc<Signer>,
    bucket: String,
}

impl Backend {
    pub(crate) fn from_iter(it: impl Iterator<Item = (String, String)>) -> Result<Self> {
        let mut builder = Builder::default();

        for (k, v) in it {
            let v = v.as_str();
            match k.as_ref() {
                "root" => builder.root(v),
                "bucket" => builder.bucket(v),
                "endpoint" => builder.endpoint(v),
                "access_key_id" => builder.access_key_id(v),
                "secret_access_key" => builder.secret_access_key(v),
                _ => continue,
            };
        }

        builder.build()
    }
}

#[async_trait]
impl Accessor for Backend {
    fn metadata(&self) -> AccessorMetadata {
        let mut am = AccessorMetadata::default();
        am.set_scheme(Scheme::Obs)
            .set_root(&self.root)
            .set_name(&self.bucket)
            .set_capabilities(
                AccessorCapability::Read | AccessorCapability::Write | AccessorCapability::List,
            );

        am
    }

    async fn create(&self, path: &str, _: OpCreate) -> Result<()> {
        let mut req = self.obs_put_object_request(path, Some(0), AsyncBody::Empty)?;

        self.signer
            .sign(&mut req)
            .map_err(|e| new_request_sign_error(Operation::Create, path, e))?;

        let resp = self
            .client
            .send_async(req)
            .await
            .map_err(|e| new_request_send_error(Operation::Write, path, e))?;

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK => {
                resp.into_body()
                    .consume()
                    .await
                    .map_err(|err| new_response_consume_error(Operation::Write, path, err))?;
                Ok(())
            }
            _ => {
                let er = parse_error_response(resp).await?;
                let err = parse_error(Operation::Create, path, er);
                Err(err)
            }
        }
    }

    async fn read(&self, path: &str, args: OpRead) -> Result<BytesReader> {
        let resp = self
            .obs_get_object(path, args.offset(), args.size())
            .await?;

        let status = resp.status();

        match status {
            StatusCode::OK | StatusCode::PARTIAL_CONTENT => Ok(resp.into_body().reader()),
            _ => {
                let er = parse_error_response(resp).await?;
                let err = parse_error(Operation::Read, path, er);
                Err(err)
            }
        }
    }

    async fn write(&self, path: &str, args: OpWrite, r: BytesReader) -> Result<u64> {
        let mut req = self.obs_put_object_request(path, Some(args.size()), AsyncBody::Reader(r))?;

        self.signer
            .sign(&mut req)
            .map_err(|e| new_request_sign_error(Operation::Write, path, e))?;

        let resp = self
            .client
            .send_async(req)
            .await
            .map_err(|e| new_request_send_error(Operation::Write, path, e))?;

        let status = resp.status();

        match status {
            StatusCode::CREATED | StatusCode::OK => {
                resp.into_body()
                    .consume()
                    .await
                    .map_err(|err| new_response_consume_error(Operation::Write, path, err))?;
                Ok(args.size())
            }
            _ => {
                let er = parse_error_response(resp).await?;
                let err = parse_error(Operation::Write, path, er);
                Err(err)
            }
        }
    }

    async fn stat(&self, path: &str, _: OpStat) -> Result<ObjectMetadata> {
        // Stat root always returns a DIR.
        if path == "/" {
            let mut m = ObjectMetadata::default();
            m.set_mode(ObjectMode::DIR);
            return Ok(m);
        }

        let resp = self.obs_get_head_object(path).await?;

        let status = resp.status();

        // The response is very similar to azblob.
        match status {
            StatusCode::OK => {
                let mut m = ObjectMetadata::default();

                if let Some(v) = parse_content_length(resp.headers())
                    .map_err(|e| other(ObjectError::new(Operation::Stat, path, e)))?
                {
                    m.set_content_length(v);
                }

                if let Some(v) = parse_etag(resp.headers())
                    .map_err(|e| other(ObjectError::new(Operation::Stat, path, e)))?
                {
                    m.set_etag(v);
                    m.set_content_md5(v.trim_matches('"'));
                }

                if let Some(v) = parse_last_modified(resp.headers())
                    .map_err(|e| other(ObjectError::new(Operation::Stat, path, e)))?
                {
                    m.set_last_modified(v);
                }

                if path.ends_with('/') {
                    m.set_mode(ObjectMode::DIR);
                } else {
                    m.set_mode(ObjectMode::FILE);
                };

                Ok(m)
            }
            StatusCode::NOT_FOUND if path.ends_with('/') => {
                let mut m = ObjectMetadata::default();
                m.set_mode(ObjectMode::DIR);

                Ok(m)
            }
            _ => {
                let er = parse_error_response(resp).await?;
                let err = parse_error(Operation::Stat, path, er);
                Err(err)
            }
        }
    }

    async fn delete(&self, path: &str, _: OpDelete) -> Result<()> {
        let resp = self.obs_delete_object(path).await?;

        let status = resp.status();

        match status {
            StatusCode::NO_CONTENT | StatusCode::ACCEPTED | StatusCode::NOT_FOUND => Ok(()),
            _ => {
                let er = parse_error_response(resp).await?;
                let err = parse_error(Operation::Delete, path, er);
                Err(err)
            }
        }
    }

    async fn list(&self, path: &str, _: OpList) -> Result<DirStreamer> {
        Ok(Box::new(DirStream::new(
            Arc::new(self.clone()),
            &self.root,
            path,
        )))
    }
}

impl Backend {
    async fn obs_get_object(
        &self,
        path: &str,
        offset: Option<u64>,
        size: Option<u64>,
    ) -> Result<Response<AsyncBody>> {
        let p = build_abs_path(&self.root, path);

        let url = format!("{}/{}", self.endpoint, percent_encode_path(&p));

        let mut req = Request::get(&url);

        if offset.is_some() || size.is_some() {
            req = req.header(
                http::header::RANGE,
                BytesRange::new(offset, size).to_string(),
            )
        }

        let mut req = req
            .body(AsyncBody::Empty)
            .map_err(|e| new_request_build_error(Operation::Read, path, e))?;

        self.signer
            .sign(&mut req)
            .map_err(|e| new_request_sign_error(Operation::Read, path, e))?;

        self.client
            .send_async(req)
            .await
            .map_err(|e| new_request_send_error(Operation::Read, path, e))
    }

    fn obs_put_object_request(
        &self,
        path: &str,
        size: Option<u64>,
        body: AsyncBody,
    ) -> Result<Request<AsyncBody>> {
        let p = build_abs_path(&self.root, path);

        let url = format!("{}/{}", self.endpoint, percent_encode_path(&p));

        let mut req = Request::put(&url);

        if let Some(size) = size {
            req = req.header(CONTENT_LENGTH, size)
        }

        let req = req
            .body(body)
            .map_err(|e| new_request_build_error(Operation::Write, path, e))?;

        Ok(req)
    }

    async fn obs_get_head_object(&self, path: &str) -> Result<Response<AsyncBody>> {
        let p = build_abs_path(&self.root, path);

        let url = format!("{}/{}", self.endpoint, percent_encode_path(&p));

        // The header 'Origin' is optional for API calling, the doc has mistake, confirmed with customer service of huaweicloud.
        // https://support.huaweicloud.com/intl/en-us/api-obs/obs_04_0084.html

        let req = Request::head(&url);

        let mut req = req
            .body(AsyncBody::Empty)
            .map_err(|e| new_request_build_error(Operation::Stat, path, e))?;

        self.signer
            .sign(&mut req)
            .map_err(|e| new_request_sign_error(Operation::Stat, path, e))?;

        self.client
            .send_async(req)
            .await
            .map_err(|e| new_request_send_error(Operation::Stat, path, e))
    }

    async fn obs_delete_object(&self, path: &str) -> Result<Response<AsyncBody>> {
        let p = build_abs_path(&self.root, path);

        let url = format!("{}/{}", self.endpoint, percent_encode_path(&p));

        let req = Request::delete(&url);

        let mut req = req
            .body(AsyncBody::Empty)
            .map_err(|e| new_request_build_error(Operation::Delete, path, e))?;

        self.signer
            .sign(&mut req)
            .map_err(|e| new_request_sign_error(Operation::Delete, path, e))?;

        self.client
            .send_async(req)
            .await
            .map_err(|e| new_request_send_error(Operation::Delete, path, e))
    }

    pub(crate) async fn obs_list_objects(
        &self,
        path: &str,
        next_marker: &str,
    ) -> Result<Response<AsyncBody>> {
        let p = build_abs_path(&self.root, path);

        let mut url = format!("{}?delimiter=/", self.endpoint);
        if !path.is_empty() {
            write!(url, "&prefix={}", percent_encode_path(&p))
                .expect("write into string must succeed");
        }
        if !next_marker.is_empty() {
            write!(url, "&marker={next_marker}").expect("write into string must succeed");
        }

        let mut req = Request::get(&url)
            .body(AsyncBody::Empty)
            .map_err(|e| new_request_build_error(Operation::List, path, e))?;

        self.signer
            .sign(&mut req)
            .map_err(|e| new_request_sign_error(Operation::List, path, e))?;

        self.client
            .send_async(req)
            .await
            .map_err(|e| new_request_send_error(Operation::List, path, e))
    }
}