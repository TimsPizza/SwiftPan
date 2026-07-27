mod bytes;
mod storage_faults;

pub(crate) use bytes::patterned_bytes;
pub(crate) use storage_faults::inject_early_eof;
