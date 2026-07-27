use crate::share::{
    load_ledger_with_cache, prepend_share_entry, save_ledger_with_cache, ShareEntry, ShareLedger,
    STATIC_SHARE_PATH,
};
use opendal::services::Memory;

fn memory_client() -> opendal::Operator {
    opendal::Operator::new(Memory::default())
        .expect("memory service should build")
        .finish()
}

fn entry(key: &str, url: &str) -> ShareEntry {
    ShareEntry {
        key: key.into(),
        url: url.into(),
        created_at_ms: 100,
        expires_at_ms: 3_700_000,
        ttl_secs: 3_600,
        download_filename: Some("photo.arw".into()),
    }
}

#[tokio::test]
async fn share_ledger_is_persisted_remotely_and_cached_locally() {
    let client = memory_client();
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let cache_path = directory.path().join("share_cache.json");
    let expected_entry = entry("camera/DSC00001.ARW", "https://example.test/signed");
    let ledger = ShareLedger {
        items: vec![expected_entry.clone()],
        updated_at_ms: 0,
    };

    save_ledger_with_cache(&client, &ledger, Some(&cache_path))
        .await
        .expect("share ledger should persist");

    let remote_bytes = client
        .read(STATIC_SHARE_PATH)
        .await
        .expect("remote share ledger should exist")
        .to_bytes();
    let remote: ShareLedger =
        serde_json::from_slice(&remote_bytes).expect("remote ledger should be valid JSON");
    assert_eq!(remote.items.len(), 1);
    assert_eq!(remote.items[0].key, expected_entry.key);
    assert_eq!(remote.items[0].url, expected_entry.url);
    assert_eq!(remote.items[0].download_filename, Some("photo.arw".into()));
    assert!(remote.updated_at_ms > 0);
    assert!(cache_path.exists());

    client
        .delete(STATIC_SHARE_PATH)
        .await
        .expect("remote fixture should delete");
    let cached = load_ledger_with_cache(&client, false, Some(&cache_path))
        .await
        .expect("fresh local cache should satisfy the read");
    assert_eq!(cached.items.len(), 1);
    assert_eq!(cached.items[0].key, expected_entry.key);
}

#[tokio::test]
async fn forced_share_refresh_treats_remote_ledger_as_authoritative() {
    let client = memory_client();
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let cache_path = directory.path().join("share_cache.json");
    let local = ShareLedger {
        items: vec![entry("stale-local.bin", "https://example.test/local")],
        updated_at_ms: chrono::Utc::now().timestamp_millis(),
    };
    std::fs::write(
        &cache_path,
        serde_json::to_vec(&local).expect("local ledger should serialize"),
    )
    .expect("local cache should write");

    let remote = ShareLedger {
        items: vec![entry("remote.arw", "https://example.test/remote")],
        updated_at_ms: 1,
    };
    client
        .write(
            STATIC_SHARE_PATH,
            serde_json::to_vec(&remote).expect("remote ledger should serialize"),
        )
        .await
        .expect("remote fixture should write");

    let cache_hit = load_ledger_with_cache(&client, false, Some(&cache_path))
        .await
        .expect("normal read should use fresh cache");
    assert_eq!(cache_hit.items[0].key, "stale-local.bin");

    let refreshed = load_ledger_with_cache(&client, true, Some(&cache_path))
        .await
        .expect("forced read should fetch remote ledger");
    assert_eq!(refreshed.items[0].key, "remote.arw");

    let rewritten_cache: ShareLedger =
        serde_json::from_slice(&std::fs::read(&cache_path).expect("refreshed cache should exist"))
            .expect("refreshed cache should be valid JSON");
    assert_eq!(rewritten_cache.items[0].key, "remote.arw");
}

#[tokio::test]
async fn recording_a_share_updates_remote_history_and_enforces_its_bound() {
    let client = memory_client();
    let directory = tempfile::tempdir().expect("temporary directory should exist");
    let cache_path = directory.path().join("share_cache.json");
    let existing = (0..1000)
        .map(|index| {
            entry(
                &format!("existing-{index}.bin"),
                &format!("https://example.test/existing/{index}"),
            )
        })
        .collect();
    save_ledger_with_cache(
        &client,
        &ShareLedger {
            items: existing,
            updated_at_ms: 0,
        },
        Some(&cache_path),
    )
    .await
    .expect("existing remote history should persist");

    let mut ledger = load_ledger_with_cache(&client, true, Some(&cache_path))
        .await
        .expect("share flow should refresh remote history");
    prepend_share_entry(
        &mut ledger,
        entry("newest.arw", "https://example.test/newest"),
    );
    save_ledger_with_cache(&client, &ledger, Some(&cache_path))
        .await
        .expect("updated remote history should persist");

    let persisted: ShareLedger = serde_json::from_slice(
        &client
            .read(STATIC_SHARE_PATH)
            .await
            .expect("remote history should remain readable")
            .to_bytes(),
    )
    .expect("remote history should remain valid JSON");
    assert_eq!(persisted.items.len(), 1000);
    assert_eq!(persisted.items[0].key, "newest.arw");
    assert_eq!(persisted.items[999].key, "existing-998.bin");
}
