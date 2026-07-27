use crate::test_support::patterned_bytes;

pub(super) fn raw_camera_fixture(payload_len: usize) -> Vec<u8> {
    let header = b"II*\0SONY-ARW-TEST\0";
    let mut bytes = Vec::with_capacity(header.len() + payload_len);
    bytes.extend_from_slice(header);
    bytes.extend(patterned_bytes(payload_len, 17));
    bytes
}
