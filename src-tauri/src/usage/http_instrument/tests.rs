use super::*;

fn classify(method: http::Method, uri: &str, headers: http::HeaderMap) -> ClassifiedAction {
    classify_s3_action(
        &method,
        &uri.parse::<http::Uri>().expect("test URI should parse"),
        &headers,
    )
    .expect("request should have an S3 usage classification")
}

fn assert_action(
    method: http::Method,
    uri: &str,
    headers: http::HeaderMap,
    expected_name: &str,
    expected_class: OpClass,
) {
    let action = classify(method, uri, headers);
    assert_eq!(action.name, expected_name);
    assert_eq!(action.class, expected_class);
}

#[test]
fn get_requests_distinguish_list_location_and_object_reads() {
    assert_action(
        http::Method::GET,
        "https://bucket.example/?list-type=2&prefix=photos",
        http::HeaderMap::new(),
        "ListObjectsV2",
        OpClass::A,
    );
    assert_action(
        http::Method::GET,
        "https://bucket.example/?location",
        http::HeaderMap::new(),
        "GetBucketLocation",
        OpClass::B,
    );
    assert_action(
        http::Method::GET,
        "https://bucket.example/photos/image.arw",
        http::HeaderMap::new(),
        "GetObject",
        OpClass::B,
    );
}

#[test]
fn head_requests_are_class_b_object_metadata_reads() {
    assert_action(
        http::Method::HEAD,
        "https://bucket.example/photos/image.arw",
        http::HeaderMap::new(),
        "HeadObject",
        OpClass::B,
    );
}

#[test]
fn put_requests_distinguish_object_part_and_copy_writes() {
    assert_action(
        http::Method::PUT,
        "https://bucket.example/photos/image.arw",
        http::HeaderMap::new(),
        "PutObject",
        OpClass::A,
    );
    assert_action(
        http::Method::PUT,
        "https://bucket.example/photos/image.arw?partNumber=3&uploadId=upload-1",
        http::HeaderMap::new(),
        "UploadPart",
        OpClass::A,
    );

    let mut copy_headers = http::HeaderMap::new();
    copy_headers.insert(
        "x-amz-copy-source",
        http::HeaderValue::from_static("/source/photo.arw"),
    );
    assert_action(
        http::Method::PUT,
        "https://bucket.example/photos/copy.arw",
        copy_headers,
        "CopyObject",
        OpClass::A,
    );
}

#[test]
fn post_requests_distinguish_multipart_lifecycle_and_bulk_delete() {
    assert_action(
        http::Method::POST,
        "https://bucket.example/photos/image.arw?uploads",
        http::HeaderMap::new(),
        "CreateMultipartUpload",
        OpClass::A,
    );
    assert_action(
        http::Method::POST,
        "https://bucket.example/photos/image.arw?uploadId=upload-1",
        http::HeaderMap::new(),
        "CompleteMultipartUpload",
        OpClass::A,
    );
    assert_action(
        http::Method::POST,
        "https://bucket.example/?delete",
        http::HeaderMap::new(),
        "DeleteObjects",
        OpClass::A,
    );
}

#[test]
fn delete_requests_distinguish_object_delete_and_multipart_abort() {
    assert_action(
        http::Method::DELETE,
        "https://bucket.example/photos/image.arw",
        http::HeaderMap::new(),
        "DeleteObject",
        OpClass::A,
    );
    assert_action(
        http::Method::DELETE,
        "https://bucket.example/photos/image.arw?uploadId=upload-1",
        http::HeaderMap::new(),
        "AbortMultipartUpload",
        OpClass::A,
    );
}

#[test]
fn unsupported_http_methods_are_not_counted_as_s3_actions() {
    let result = classify_s3_action(
        &http::Method::PATCH,
        &"https://bucket.example/object"
            .parse::<http::Uri>()
            .expect("test URI should parse"),
        &http::HeaderMap::new(),
    );

    assert!(result.is_none());
}
