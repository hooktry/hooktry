use std::fs;

use hooktry::{
    keyring::VersionedKeyring,
    secret::{SecretError, SecretStore},
};
use uuid::Uuid;

#[test]
fn sqlite_secret_survives_restart_and_plaintext_is_absent() {
    let path = std::env::temp_dir().join(format!("hooktry-secret-{}.db", Uuid::now_v7()));
    let key = [23u8; 32];
    let workspace = Uuid::now_v7();
    let plaintext = "hooktry_token_that_must_never_be_stored_plaintext";

    let store = SecretStore::open(&path, key).unwrap();
    let first = store.put(workspace, "api-token", plaintext).unwrap();
    assert_eq!(store.resolve(workspace, "api-token").unwrap(), plaintext);
    drop(store);

    let bytes = fs::read(&path).unwrap();
    assert!(
        !bytes
            .windows(plaintext.len())
            .any(|window| window == plaintext.as_bytes())
    );

    let reopened = SecretStore::open(&path, key).unwrap();
    assert_eq!(reopened.resolve(workspace, "api-token").unwrap(), plaintext);
    assert_eq!(
        reopened.get_ref(workspace, "api-token").unwrap().id,
        first.id
    );

    let wrong_workspace = Uuid::now_v7();
    assert_eq!(
        reopened.resolve(wrong_workspace, "api-token"),
        Err(SecretError::NotFound)
    );

    drop(reopened);
    let wrong_key = SecretStore::open(&path, [24u8; 32]).unwrap();
    assert_eq!(
        wrong_key.resolve(workspace, "api-token"),
        Err(SecretError::Crypto)
    );

    let _ = fs::remove_file(path);
}

#[test]
fn master_key_rotation_reads_old_secrets_and_writes_new_version() {
    let path =
        std::env::temp_dir().join(format!("hooktry-secret-key-rotation-{}.db", Uuid::now_v7()));
    let workspace = Uuid::now_v7();
    let v1 = [0x11; 32];
    let v2 = [0x22; 32];

    let old_store = SecretStore::open(&path, v1).unwrap();
    old_store.put(workspace, "old-token", "old-secret").unwrap();
    assert_eq!(
        old_store
            .metadata(workspace, "old-token")
            .unwrap()
            .unwrap()
            .key_version,
        1
    );
    drop(old_store);

    let rotated_keys = VersionedKeyring::new(2, v2, [(1, v1)]).unwrap();
    let rotated = SecretStore::open_with_keyring(&path, rotated_keys).unwrap();
    assert_eq!(
        rotated.resolve(workspace, "old-token").unwrap(),
        "old-secret"
    );

    rotated.put(workspace, "new-token", "new-secret").unwrap();
    assert_eq!(
        rotated
            .metadata(workspace, "new-token")
            .unwrap()
            .unwrap()
            .key_version,
        2
    );
    assert_eq!(
        rotated.resolve(workspace, "new-token").unwrap(),
        "new-secret"
    );
    drop(rotated);

    let without_v1 =
        SecretStore::open_with_keyring(&path, VersionedKeyring::new(2, v2, []).unwrap()).unwrap();
    assert_eq!(
        without_v1.resolve(workspace, "old-token"),
        Err(SecretError::InvalidKey)
    );
    assert_eq!(
        without_v1.resolve(workspace, "new-token").unwrap(),
        "new-secret"
    );

    let _ = fs::remove_file(path);
}

#[test]
fn overwrite_rotates_ciphertext_and_reference() {
    let path = std::env::temp_dir().join(format!("hooktry-secret-{}.db", Uuid::now_v7()));
    let store = SecretStore::open(&path, [31u8; 32]).unwrap();
    let workspace = Uuid::now_v7();

    let first = store.put(workspace, "api-token", "first-value").unwrap();
    let second = store.put(workspace, "api-token", "second-value").unwrap();

    assert_ne!(first.id, second.id);
    assert_eq!(
        store.resolve(workspace, "api-token").unwrap(),
        "second-value"
    );
    assert_eq!(store.get_ref(workspace, "api-token").unwrap().id, second.id);

    let bytes = fs::read(&path).unwrap();
    for plaintext in ["first-value", "second-value"] {
        assert!(
            !bytes
                .windows(plaintext.len())
                .any(|window| window == plaintext.as_bytes())
        );
    }

    let _ = fs::remove_file(path);
}

#[test]
fn bound_secret_origin_survives_restart_and_fails_closed_elsewhere() {
    let path = std::env::temp_dir().join(format!("hooktry-secret-bound-{}.db", Uuid::now_v7()));
    let workspace = Uuid::now_v7();
    let store = SecretStore::open(&path, [53u8; 32]).unwrap();

    let reference = store
        .put_bound(
            workspace,
            "provider-token",
            "hidden-value",
            "https://api.example.com/v1/token",
        )
        .unwrap();
    assert_eq!(
        reference.allowed_origin.as_deref(),
        Some("https://api.example.com")
    );
    assert_eq!(
        store
            .resolve_for_origin(workspace, "provider-token", "https://api.example.com/other")
            .unwrap(),
        "hidden-value"
    );
    assert_eq!(
        store.resolve_for_origin(workspace, "provider-token", "https://other.example.com"),
        Err(SecretError::DestinationDenied)
    );

    drop(store);
    let reopened = SecretStore::open(&path, [53u8; 32]).unwrap();
    let reopened_ref = reopened.get_ref(workspace, "provider-token").unwrap();
    assert_eq!(
        reopened_ref.allowed_origin.as_deref(),
        Some("https://api.example.com")
    );

    let _ = fs::remove_file(path);
}

#[test]
fn rotation_preserves_secret_origin_binding() {
    let workspace = Uuid::now_v7();
    let store = SecretStore::default();
    store
        .put_bound(
            workspace,
            "provider-token",
            "first-value",
            "https://api.example.com",
        )
        .unwrap();

    let rotated = store
        .rotate(workspace, "provider-token", "second-value")
        .unwrap();

    assert_eq!(
        rotated.allowed_origin.as_deref(),
        Some("https://api.example.com")
    );
    assert_eq!(
        store
            .resolve_for_origin(workspace, "provider-token", "https://api.example.com")
            .unwrap(),
        "second-value"
    );
}

#[test]
fn delete_clears_secret_origin_binding() {
    let path =
        std::env::temp_dir().join(format!("hooktry-secret-delete-bound-{}.db", Uuid::now_v7()));
    let workspace = Uuid::now_v7();
    let store = SecretStore::open(&path, [61u8; 32]).unwrap();

    store
        .put_bound(
            workspace,
            "reused-name",
            "first-value",
            "https://api.example.com",
        )
        .unwrap();
    assert!(store.delete(workspace, "reused-name").unwrap());

    let replacement = store
        .put(workspace, "reused-name", "replacement-value")
        .unwrap();
    assert_eq!(replacement.allowed_origin, None);
    assert_eq!(
        store.resolve_for_origin(workspace, "reused-name", "https://api.example.com"),
        Err(SecretError::DestinationDenied)
    );

    let _ = fs::remove_file(path);
}
