use super::super::*;
use crate::test_support::patterned_bytes;

fn assert_ranges_reconstruct_original(total: usize, chunk_size: u64) {
    let original = patterned_bytes(total, 5);
    let mut offset = 0;
    let mut previous_end = 0;
    let mut reconstructed = Vec::with_capacity(original.len());

    while let Some(range) = next_download_range(offset, total as u64, chunk_size) {
        assert_eq!(range.start, previous_end, "range contains a gap or overlap");
        assert!(range.start < range.end, "range must make progress");
        assert!(range.end <= total as u64, "range exceeds object length");

        reconstructed.extend_from_slice(&original[range.start as usize..range.end as usize]);
        previous_end = range.end;
        offset = range.end;
    }

    assert_eq!(offset, total as u64);
    assert_eq!(reconstructed, original);
}

#[test]
fn range_boundaries_cover_empty_exact_and_odd_sized_objects() {
    const CHUNK: usize = 64 * 1024 + 3;

    for total in [0, 1, CHUNK - 1, CHUNK, CHUNK + 1, 2 * CHUNK, 2 * CHUNK + 17] {
        assert_ranges_reconstruct_original(total, CHUNK as u64);
    }
}

#[test]
fn range_generation_stops_at_or_beyond_total_and_rejects_zero_chunk_size() {
    assert_eq!(next_download_range(10, 10, 4), None);
    assert_eq!(next_download_range(11, 10, 4), None);
    assert_eq!(next_download_range(0, 10, 0), None);
}

#[test]
fn final_range_is_clamped_without_integer_overflow() {
    assert_eq!(
        next_download_range(u64::MAX - 2, u64::MAX, 10),
        Some((u64::MAX - 2)..u64::MAX)
    );
}
