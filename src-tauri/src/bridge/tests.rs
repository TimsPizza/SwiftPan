use super::*;

#[test]
fn access_keys_keep_only_the_first_four_ascii_characters() {
    assert_eq!(redact_key("ABCD123456"), "ABCD******");
    assert_eq!(redact_key("12345"), "1234*");
}

#[test]
fn short_access_keys_are_fully_redacted() {
    for key in ["", "1", "12", "123", "1234"] {
        assert_eq!(redact_key(key), "****");
    }
}

#[test]
fn cloudflare_endpoint_redaction_hides_account_identifier() {
    assert_eq!(
        redact_endpoint("https://account-id.r2.cloudflarestorage.com"),
        "https://*****.r2.cloudflarestorage.com"
    );
}

#[test]
fn endpoint_redaction_handles_non_https_and_hostless_values() {
    assert_eq!(
        redact_endpoint("account-id.r2.cloudflarestorage.com"),
        "*****.r2.cloudflarestorage.com"
    );
    assert_eq!(redact_endpoint("localhost"), "*****");
}
