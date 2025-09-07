use crate::types::{SpResult, UsageDelta};
use crate::usage::UsageSync;
use futures::TryStreamExt;
use opendal::raw::{HttpBody, HttpFetch, MaybeSend};
use opendal::{Buffer, Error, ErrorKind as OdErrorKind, Result as OdResult};
use std::future;
// (no atomic state used here)

// Always-on instrumentation: no runtime toggle.

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum OpClass {
    A,
    B,
}

#[derive(Clone, Debug)]
struct ClassifiedAction {
    name: &'static str,
    class: OpClass,
}

fn classify_s3_action(
    method: &http::Method,
    uri: &http::Uri,
    headers: &http::HeaderMap,
) -> Option<ClassifiedAction> {
    let q = uri.query().unwrap_or("");
    match *method {
        http::Method::GET => {
            if q.contains("list-type=") {
                Some(ClassifiedAction {
                    name: "ListObjectsV2",
                    class: OpClass::A,
                })
            } else if q.contains("location") {
                Some(ClassifiedAction {
                    name: "GetBucketLocation",
                    class: OpClass::B,
                })
            } else {
                Some(ClassifiedAction {
                    name: "GetObject",
                    class: OpClass::B,
                })
            }
        }
        http::Method::HEAD => Some(ClassifiedAction {
            name: "HeadObject",
            class: OpClass::B,
        }),
        http::Method::PUT => {
            if q.contains("partNumber=") && q.contains("uploadId=") {
                Some(ClassifiedAction {
                    name: "UploadPart",
                    class: OpClass::A,
                })
            } else if headers.contains_key("x-amz-copy-source") {
                Some(ClassifiedAction {
                    name: "CopyObject",
                    class: OpClass::A,
                })
            } else {
                Some(ClassifiedAction {
                    name: "PutObject",
                    class: OpClass::A,
                })
            }
        }
        http::Method::POST => {
            if q.contains("uploads") {
                Some(ClassifiedAction {
                    name: "CreateMultipartUpload",
                    class: OpClass::A,
                })
            } else if q.contains("uploadId=") {
                Some(ClassifiedAction {
                    name: "CompleteMultipartUpload",
                    class: OpClass::A,
                })
            } else if q.contains("delete") {
                Some(ClassifiedAction {
                    name: "DeleteObjects",
                    class: OpClass::A,
                })
            } else {
                None
            }
        }
        http::Method::DELETE => {
            if q.contains("uploadId=") {
                Some(ClassifiedAction {
                    name: "AbortMultipartUpload",
                    class: OpClass::A,
                })
            } else {
                Some(ClassifiedAction {
                    name: "DeleteObject",
                    class: OpClass::A,
                })
            }
        }
        _ => None,
    }
}

fn record_usage(
    action: &ClassifiedAction,
    ingress: u64,
    egress: u64,
    added: u64,
    deleted: u64,
) -> SpResult<()> {
    let mut a = std::collections::HashMap::new();
    let mut b = std::collections::HashMap::new();
    match action.class {
        OpClass::A => {
            a.insert(action.name.into(), 1u64);
        }
        OpClass::B => {
            b.insert(action.name.into(), 1u64);
        }
    }
    crate::logger::debug("usage", &format!("record_usage: {:?}", action));
    UsageSync::record_local_delta(UsageDelta {
        class_a: a,
        class_b: b,
        ingress_bytes: ingress,
        egress_bytes: egress,
        added_storage_bytes: added,
        deleted_storage_bytes: deleted,
    })
}

/// A reqwest-based HttpFetch that instruments S3 calls for Class A/B counting.
#[derive(Clone)]
pub struct InstrumentedReqwest {
    inner: reqwest::Client,
}

impl InstrumentedReqwest {
    pub fn new(inner: reqwest::Client) -> Self {
        Self { inner }
    }
}

impl HttpFetch for InstrumentedReqwest {
    fn fetch(
        &self,
        req: http::Request<Buffer>,
    ) -> impl future::Future<Output = OdResult<http::Response<HttpBody>>> + MaybeSend {
        let client = self.inner.clone();
        async move {
            // Clone uri & method for context and classification.
            let uri = req.uri().clone();
            let method = req.method().clone();
            let (parts, body) = req.into_parts();

            // Build reqwest request
            let url = reqwest::Url::parse(&uri.to_string()).map_err(|err| {
                Error::new(OdErrorKind::Unexpected, "request url is invalid")
                    .with_operation("http_util::Client::send::fetch")
                    .with_context("url", uri.to_string())
                    .set_source(err)
            })?;

            let mut req_builder = client
                .request(parts.method.clone(), url)
                .headers(parts.headers.clone());
            #[cfg(not(target_arch = "wasm32"))]
            {
                req_builder = req_builder.version(parts.version);
            }
            if !body.is_empty() {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    // Wrap Buffer into reqwest body via HttpBufferBody-like path
                    struct HttpBufferBody(Buffer);
                    impl http_body::Body for HttpBufferBody {
                        type Data = bytes::Bytes;
                        type Error = std::convert::Infallible;
                        fn poll_frame(
                            mut self: std::pin::Pin<&mut Self>,
                            _: &mut std::task::Context<'_>,
                        ) -> std::task::Poll<
                            Option<Result<http_body::Frame<Self::Data>, Self::Error>>,
                        > {
                            match ::std::iter::Iterator::next(&mut self.0) {
                                Some(bs) => {
                                    std::task::Poll::Ready(Some(Ok(http_body::Frame::data(bs))))
                                }
                                None => std::task::Poll::Ready(None),
                            }
                        }
                        fn is_end_stream(&self) -> bool {
                            self.0.is_empty()
                        }
                        fn size_hint(&self) -> http_body::SizeHint {
                            http_body::SizeHint::with_exact(self.0.len() as u64)
                        }
                    }
                    req_builder =
                        req_builder.body(reqwest::Body::wrap(HttpBufferBody(body.clone())));
                }
                #[cfg(target_arch = "wasm32")]
                {
                    req_builder = req_builder.body(reqwest::Body::from(body.to_bytes()));
                }
            }

            // Execute
            let mut resp = req_builder.send().await.map_err(|err| {
                Error::new(OdErrorKind::Unexpected, "send http request")
                    .with_operation("http_util::Client::send")
                    .with_context("url", uri.to_string())
                    .set_source(err)
            })?;

            // Determine expected content length for response body if applicable
            let is_head = method == http::Method::HEAD;
            let content_length =
                if is_head || opendal::raw::parse_content_encoding(resp.headers())?.is_some() {
                    None
                } else {
                    opendal::raw::parse_content_length(resp.headers())?
                };

            // Build response parts
            let mut hr = http::Response::builder()
                .status(resp.status())
                .extension(uri.clone());
            #[cfg(not(target_arch = "wasm32"))]
            {
                hr = hr.version(resp.version());
            }
            std::mem::swap(hr.headers_mut().unwrap(), resp.headers_mut());

            // Instrumentation: classify and record usage based on request (always on)
            let mut is_get_object = false;
            if let Some(action) = classify_s3_action(&method, &uri, &parts.headers) {
                // Upload (ingress/storage) for PUT/POST with payload
                let mut ingress_bytes: u64 = 0;
                let mut added_bytes: u64 = 0;
                match action.name {
                    "PutObject" | "UploadPart" => {
                        let sz = body.len() as u64;
                        ingress_bytes = ingress_bytes.saturating_add(sz);
                        added_bytes = added_bytes.saturating_add(sz);
                    }
                    "GetObject" => {
                        is_get_object = true;
                    }
                    _ => {}
                }
                let _ = record_usage(&action, ingress_bytes, 0, added_bytes, 0);
            }

            // Construct streaming body
            let stream = resp
                .bytes_stream()
                .try_filter(|v| future::ready(!v.is_empty()))
                .map_ok(move |bs| {
                    if is_get_object {
                        let _ = UsageSync::record_local_delta(UsageDelta {
                            class_a: Default::default(),
                            class_b: Default::default(),
                            ingress_bytes: 0,
                            egress_bytes: bs.len() as u64,
                            added_storage_bytes: 0,
                            deleted_storage_bytes: 0,
                        });
                    }
                    Buffer::from(bs)
                })
                .map_err(move |err| {
                    Error::new(OdErrorKind::Unexpected, "read data from http response")
                        .with_operation("http_util::Client::send")
                        .with_context("url", uri.to_string())
                        .set_source(err)
                });
            let body = HttpBody::new(stream, content_length);
            let resp = hr.body(body).expect("response must build succeed");
            Ok(resp)
        }
    }
}
