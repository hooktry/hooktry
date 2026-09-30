use ortyo::secret::{SecretError, SecretStore};
use uuid::Uuid;

#[test]
fn explicit_rotation_replaces_value_and_reference() {
    let store = SecretStore::default();
    let workspace = Uuid::now_v7();

    let first = store.put(workspace, "api-token", "first-value").unwrap();
    let second = store
        .rotate(workspace, "api-token", "second-value")
        .unwrap();

    assert_ne!(first.id, second.id);
    assert_eq!(second.workspace_id, workspace);
    assert_eq!(second.name, "api-token");
    assert_eq!(
        store.resolve(workspace, "api-token").unwrap(),
        "second-value"
    );

    let metadata = store.metadata(workspace, "api-token").unwrap().unwrap();
    assert_eq!(metadata.reference, second);
    assert_eq!(metadata.key_version, 1);

    let debug = format!("{metadata:?}");
    assert!(!debug.contains("first-value"));
    assert!(!debug.contains("second-value"));
}

#[test]
fn deletion_is_workspace_scoped_and_idempotent() {
    let store = SecretStore::default();
    let owner = Uuid::now_v7();
    let other = Uuid::now_v7();

    store.put(owner, "shared-name", "owner-value").unwrap();
    store.put(other, "shared-name", "other-value").unwrap();

    assert!(store.delete(owner, "shared-name").unwrap());
    assert!(!store.delete(owner, "shared-name").unwrap());
    assert_eq!(
        store.resolve(owner, "shared-name"),
        Err(SecretError::NotFound)
    );
    assert_eq!(
        store.resolve(other, "shared-name").unwrap(),
        "other-value"
    );
}

#[test]
fn metadata_listing_is_workspace_scoped_sorted_and_non_secret() {
    let store = SecretStore::default();
    let workspace = Uuid::now_v7();
    let other = Uuid::now_v7();

    store.put(workspace, "z-token", "z-secret-value").unwrap();
    store.put(workspace, "a-token", "a-secret-value").unwrap();
    store.put(other, "hidden", "other-secret-value").unwrap();

    let metadata = store.list_metadata(workspace).unwrap();
    assert_eq!(metadata.len(), 2);
    assert_eq!(metadata[0].reference.name, "a-token");
    assert_eq!(metadata[1].reference.name, "z-token");
    assert!(metadata.iter().all(|item| item.reference.workspace_id == workspace));
    assert!(metadata.iter().all(|item| item.key_version == 1));

    let debug = format!("{metadata:?}");
    for secret in ["a-secret-value", "z-secret-value", "other-secret-value"] {
        assert!(!debug.contains(secret));
    }
}

#[tokio::test]
async fn async_lifecycle_keeps_blocking_backends_off_tokio_workers() {
    let store = SecretStore::default();
    let workspace = Uuid::now_v7();

    let first = store
        .put_async(
            workspace,
            "runtime-capability".to_owned(),
            "capability-v1".to_owned(),
        )
        .await
        .unwrap();
    let second = store
        .rotate_async(
            workspace,
            "runtime-capability".to_owned(),
            "capability-v2".to_owned(),
        )
        .await
        .unwrap();

    assert_ne!(first.id, second.id);
    assert_eq!(
        store
            .resolve_async(workspace, "runtime-capability".to_owned())
            .await
            .unwrap(),
        "capability-v2"
    );

    let metadata = store.list_metadata_async(workspace).await.unwrap();
    assert_eq!(metadata.len(), 1);
    assert_eq!(metadata[0].reference, second);

    assert!(
        store
            .delete_async(workspace, "runtime-capability".to_owned())
            .await
            .unwrap()
    );
    assert_eq!(
        store
            .resolve_async(workspace, "runtime-capability".to_owned())
            .await,
        Err(SecretError::NotFound)
    );
}
