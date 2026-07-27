/// Generates deterministic, non-uniform bytes for boundary and integrity tests.
///
/// This is deliberately reproducible: a failing case must not depend on hidden
/// randomness.
pub(crate) fn patterned_bytes(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|index| {
            let position = index as u64;
            position
                .wrapping_mul(31)
                .wrapping_add(u64::from(seed))
                .wrapping_add(position.rotate_left(7)) as u8
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_bytes_are_reproducible_and_respect_requested_length() {
        let first = patterned_bytes(1025, 42);
        let second = patterned_bytes(1025, 42);

        assert_eq!(first.len(), 1025);
        assert_eq!(first, second);
    }

    #[test]
    fn changing_seed_changes_generated_bytes() {
        assert_ne!(patterned_bytes(1025, 1), patterned_bytes(1025, 2));
    }
}
