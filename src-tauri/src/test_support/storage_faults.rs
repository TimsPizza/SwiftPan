//! Deterministic storage doubles for cross-module failure-path tests.
//!
//! These layers alter only a narrow OpenDAL contract at SwiftPan's storage
//! boundary. They must not duplicate application business logic or pretend to
//! emulate all of S3.

use opendal::raw::{
    oio, Access, Layer, LayeredAccess, OpList, OpRead, OpWrite, RpDelete, RpList, RpRead, RpWrite,
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
