window.SIDEBAR_ITEMS = {"constant":[["VERSION","VERSION is the compiled version of OpenDAL."]],"enum":[["AccessorCapability","AccessorCapability describes accessor’s advanced capability."],["AccessorHint","AccessorHint describes accessor’s hint."],["AsyncBody","Body used in async HTTP requests."],["BatchedReply","Batch results of `batch` operations."],["Operation","Operation is the name for APIs in `Accessor`."]],"fn":[["build_abs_path","build_abs_path will build an absolute path with root."],["build_header_value","Build header value from given string."],["build_rel_path","build_rel_path will build a relative path towards root."],["build_rooted_abs_path","build_rooted_abs_path will build an absolute path with root."],["format_authorization_by_basic","format authorization header by basic auth."],["format_authorization_by_bearer","format authorization header by bearer token."],["format_content_md5","format content md5 header by given input."],["get_basename","Get basename from path."],["get_parent","Get parent from path."],["new_json_deserialize_error","Parse json deserialize error into opendal::Error."],["new_json_serialize_error","Parse json serialize error into opendal::Error."],["new_request_build_error","Create a new error happened during building request."],["new_request_credential_error","Create a new error happened during signing request."],["new_request_sign_error","Create a new error happened during signing request."],["new_xml_deserialize_error","Parse xml deserialize error into opendal::Error."],["normalize_path","Make sure all operation are constructed by normalized path:"],["normalize_root","Make sure root is normalized to style like `/abc/def/`."],["parse_content_disposition","Parse Content-Disposition for header map"],["parse_content_length","Parse content length from header map."],["parse_content_md5","Parse content md5 from header map."],["parse_content_range","Parse content range from header map."],["parse_content_type","Parse content type from header map."],["parse_datetime_from_from_timestamp_millis","parse datetime from given timestamp_millis"],["parse_datetime_from_rfc2822","Parse dateimt from rfc2822."],["parse_datetime_from_rfc3339","Parse dateimt from rfc3339."],["parse_error_response","parse_error_response will parse response into `ErrorResponse`."],["parse_etag","Parse etag from header map."],["parse_into_metadata","parse_into_metadata will parse standards http headers into Metadata."],["parse_last_modified","Parse last modified from header map."],["parse_location","Parse redirect location from header map"],["percent_encode_path","percent_encode_path will do percent encoding for http encode path."],["validate_path","Validate given path is match with given EntryMode."]],"mod":[["adapters","Providing adapters and its implementations."],["oio","`oio` provides OpenDAL’s raw traits and types that opendal returns as output."]],"struct":[["AccessorInfo","Metadata for accessor, users can use this metadata to get information of underlying backend."],["BytesContentRange","BytesContentRange is the content range of bytes."],["BytesRange","BytesRange(offset, size) carries a range of content."],["ErrorResponse","ErrorResponse carries HTTP status code, headers and body."],["HttpClient","HttpClient that used across opendal."],["IncomingAsyncBody","IncomingAsyncBody carries the content returned by remote servers."],["PresignedRequest","PresignedRequest is a presigned request return by `presign`."],["RpBatch","Reply for `batch` operation."],["RpCopy","Reply for `copy` operation."],["RpCreate","Reply for `create` operation"],["RpDelete","Reply for `delete` operation"],["RpList","Reply for `list` operation."],["RpPresign","Reply for `presign` operation."],["RpRead","Reply for `read` operation."],["RpRename","Reply for `rename` operation."],["RpScan","Reply for `scan` operation."],["RpStat","Reply for `stat` operation."],["RpWrite","Reply for `write` operation."]],"trait":[["Accessor","Underlying trait of all backends for implementors."],["Layer","Layer is used to intercept the operations on the underlying storage."],["LayeredAccessor","LayeredAccessor is layered accessor that forward all not implemented method to inner."]],"type":[["FusedAccessor","FusedAccessor is the type erased accessor with `Box<dyn Read>`."]]};