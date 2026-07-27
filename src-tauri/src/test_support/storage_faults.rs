//! Deterministic storage doubles for cross-module failure-path tests.
//!
//! These layers alter only a narrow OpenDAL contract at SwiftPan's storage
//! boundary. They must not duplicate application business logic or pretend to
//! emulate all of S3.

use opendal::raw::{
    oio, Access, BytesRange, Layer, LayeredAccess, OpList, OpRead, OpStat, OpWrite, RpDelete,
    RpList, RpRead, RpStat, RpWrite,
};
use opendal::{Buffer, Operator};

#[derive(Debug, Clone)]
struct EarlyEofLayer {
    cutoff: u64,
}

impl<A: Access> Layer<A> for EarlyEofLayer {
    type LayeredAccess = EarlyEofAccessor<A>;

    fn layer(&self, inner: A) -> Self::LayeredAccess {
        EarlyEofAccessor {
            inner,
            cutoff: self.cutoff,
        }
    }
}

#[derive(Debug)]
struct EarlyEofAccessor<A> {
    inner: A,
    cutoff: u64,
}

impl<A: Access> LayeredAccess for EarlyEofAccessor<A> {
    type Inner = A;
    type Reader = oio::Reader;
    type Writer = A::Writer;
    type Lister = A::Lister;
    type Deleter = A::Deleter;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    async fn read(&self, path: &str, args: OpRead) -> opendal::Result<(RpRead, Self::Reader)> {
        if args.range().offset() >= self.cutoff {
            return Ok((RpRead::new(), Box::new(Buffer::new())));
        }
        let (response, reader) = self.inner.read(path, args).await?;
        Ok((response, Box::new(reader)))
    }

    async fn write(&self, path: &str, args: OpWrite) -> opendal::Result<(RpWrite, Self::Writer)> {
        self.inner.write(path, args).await
    }

    async fn list(&self, path: &str, args: OpList) -> opendal::Result<(RpList, Self::Lister)> {
        self.inner.list(path, args).await
    }

    async fn delete(&self) -> opendal::Result<(RpDelete, Self::Deleter)> {
        self.inner.delete().await
    }
}

pub(crate) fn inject_early_eof(operator: Operator, cutoff: u64) -> Operator {
    operator.layer(EarlyEofLayer { cutoff })
}

#[derive(Debug, Clone)]
struct ShortReadLayer {
    max_bytes: u64,
}

impl<A: Access> Layer<A> for ShortReadLayer {
    type LayeredAccess = ShortReadAccessor<A>;

    fn layer(&self, inner: A) -> Self::LayeredAccess {
        ShortReadAccessor {
            inner,
            max_bytes: self.max_bytes,
        }
    }
}

#[derive(Debug)]
struct ShortReadAccessor<A> {
    inner: A,
    max_bytes: u64,
}

impl<A: Access> LayeredAccess for ShortReadAccessor<A> {
    type Inner = A;
    type Reader = A::Reader;
    type Writer = A::Writer;
    type Lister = A::Lister;
    type Deleter = A::Deleter;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    async fn read(&self, path: &str, args: OpRead) -> opendal::Result<(RpRead, Self::Reader)> {
        let range = args.range();
        let limited = range.size().unwrap_or(self.max_bytes).min(self.max_bytes);
        self.inner
            .read(
                path,
                args.with_range(BytesRange::new(range.offset(), Some(limited))),
            )
            .await
    }

    async fn write(&self, path: &str, args: OpWrite) -> opendal::Result<(RpWrite, Self::Writer)> {
        self.inner.write(path, args).await
    }

    async fn list(&self, path: &str, args: OpList) -> opendal::Result<(RpList, Self::Lister)> {
        self.inner.list(path, args).await
    }

    async fn delete(&self) -> opendal::Result<(RpDelete, Self::Deleter)> {
        self.inner.delete().await
    }
}

pub(crate) fn limit_read_responses(operator: Operator, max_bytes: u64) -> Operator {
    operator.layer(ShortReadLayer { max_bytes })
}

#[derive(Debug, Clone)]
struct EtagLayer {
    etag: String,
}

impl<A: Access> Layer<A> for EtagLayer {
    type LayeredAccess = EtagAccessor<A>;

    fn layer(&self, inner: A) -> Self::LayeredAccess {
        EtagAccessor {
            inner,
            etag: self.etag.clone(),
        }
    }
}

#[derive(Debug)]
struct EtagAccessor<A> {
    inner: A,
    etag: String,
}

impl<A: Access> LayeredAccess for EtagAccessor<A> {
    type Inner = A;
    type Reader = A::Reader;
    type Writer = A::Writer;
    type Lister = A::Lister;
    type Deleter = A::Deleter;

    fn inner(&self) -> &Self::Inner {
        &self.inner
    }

    async fn stat(&self, path: &str, args: OpStat) -> opendal::Result<RpStat> {
        let etag = self.etag.clone();
        self.inner.stat(path, args).await.map(|response| {
            response.map_metadata(|mut metadata| {
                metadata.set_etag(&etag);
                metadata
            })
        })
    }

    async fn read(&self, path: &str, args: OpRead) -> opendal::Result<(RpRead, Self::Reader)> {
        self.inner.read(path, args).await
    }

    async fn write(&self, path: &str, args: OpWrite) -> opendal::Result<(RpWrite, Self::Writer)> {
        self.inner.write(path, args).await
    }

    async fn list(&self, path: &str, args: OpList) -> opendal::Result<(RpList, Self::Lister)> {
        self.inner.list(path, args).await
    }

    async fn delete(&self) -> opendal::Result<(RpDelete, Self::Deleter)> {
        self.inner.delete().await
    }
}

pub(crate) fn report_etag(operator: Operator, etag: &str) -> Operator {
    operator.layer(EtagLayer {
        etag: etag.to_string(),
    })
}
