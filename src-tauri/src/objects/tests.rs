use super::*;
use opendal::services::Memory;

fn memory_operator() -> Operator {
    Operator::new(Memory::default())
        .expect("memory service should build")
        .finish()
}

#[tokio::test]
async fn flat_listing_projects_storage_entries_into_swiftpan_files() {
    let operator = memory_operator();
    let object_key = "photos/camera.arw";
    let thumbnail_key = thumbnail::thumbnail_key_for(object_key);
    operator
        .write(object_key, vec![1, 2, 3])
        .await
        .expect("object fixture should write");
    operator
        .write(&thumbnail_key, vec![4, 5])
        .await
        .expect("thumbnail fixture should write");
    operator
        .write("analytics/daily/2026-07-27.json", vec![6])
        .await
        .expect("analytics fixture should write");

    let items = list_all_objects(&operator, 100)
        .await
        .expect("listing should succeed");

    assert_eq!(items.len(), 2, "thumbnail objects must stay hidden");
    let object = items
        .iter()
        .find(|item| item.key == object_key)
        .expect("user object should be projected");
    assert_eq!(
        object.thumbnail_key.as_deref(),
        Some(thumbnail_key.as_str())
    );
    assert!(!object.protected);
    let analytics = items
        .iter()
        .find(|item| item.key.starts_with(ANALYTICS_PREFIX))
        .expect("analytics object should remain visible");
    assert!(analytics.protected);
}

#[tokio::test(flavor = "multi_thread")]
async fn deleting_user_object_also_removes_its_thumbnail_but_protects_analytics() {
    let operator = memory_operator();
    let object_key = "photos/delete-me.jpg";
    let thumbnail_key = thumbnail::thumbnail_key_for(object_key);
    operator
        .write(object_key, vec![1])
        .await
        .expect("object fixture should write");
    operator
        .write(&thumbnail_key, vec![2])
        .await
        .expect("thumbnail fixture should write");

    assert_eq!(
        delete_object(&operator, object_key)
            .await
            .expect("delete should succeed"),
        object_key
    );
    assert!(operator.stat(object_key).await.is_err());
    assert!(operator.stat(&thumbnail_key).await.is_err());

    let error = delete_object(&operator, "analytics/daily/protected.json")
        .await
        .expect_err("analytics object must be protected");
    assert!(matches!(error.kind, ErrorKind::NotRetriable));
}
