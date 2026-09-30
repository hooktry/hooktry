use std::fs;

use ortyo::secret::{SecretError, SecretStore};
use uuid::Uuid;

#[test]
fn sqlite_secret_survives_restart_and_plaintext_is_absent() {
    let path = std::env::temp_dir().join(format!("ortyo-secret-{}.db", Uuid::now_v7()));
    let key = [23u8; 32];
    let workspace = Uuid::now_v7();
    let plaintext = "ortyo_token_that_must_never_be_stored_plaintext";

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
    assert_eq!(reopened.get_ref(workspace, "api-token").unwrap().id, first.id);

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
fn overwrite_rotates_ciphertext_and_reference() {
    let path = std::env::temp_dir().join(format!("ortyo-secret-{}.db", Uuid::now_v7()));
    let store = SecretStore::open(&path, [31u8; 32]).unwrap();
    let workspace = Uuid::now_v7();

    let first = store.put(workspace, "api-token", "first-value").unwrap();
    let second = store.put(workspace, "api-token", "second-value").unwrap();

    assert_ne!(first.id, second.id);
    assert_eq!(store.resolve(workspace, "api-token").unwrap(), "second-value");
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
