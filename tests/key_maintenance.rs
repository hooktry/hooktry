use std::{collections::BTreeMap, fs};

use hooktry::{
    approval::{ApprovalDecision, ApprovalStore, derive_digest_key, derive_digest_keyring},
    execution::HttpExecutionRequest,
    key_maintenance::{KeyMaintenance, KeyMaintenanceConfig},
    keyring::VersionedKeyring,
    secret::SecretStore,
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn rewrap_then_terminal_approval_makes_old_key_safe_to_retire() {
    let path = std::env::temp_dir().join(format!("hooktry-key-maintenance-{}.db", Uuid::now_v7()));
    let workspace = Uuid::now_v7();
    let requester = Uuid::now_v7();
    let approver = Uuid::now_v7();
    let root_v1 = [0x11; 32];
    let root_v2 = [0x22; 32];

    let v1_secrets = SecretStore::open(&path, root_v1).unwrap();
    let original = v1_secrets
        .put_bound(
            workspace,
            "provider-token",
            "hidden-value",
            "https://api.example.com/v1",
        )
        .unwrap();
    drop(v1_secrets);

    let v1_approvals = ApprovalStore::open(&path, derive_digest_key(&root_v1)).unwrap();
    let request = http_request();
    let approval = v1_approvals.create(workspace, requester, &request).unwrap();
    drop(v1_approvals);

    let keyring = VersionedKeyring::new(2, root_v2, [(1, root_v1)]).unwrap();
    let maintenance = KeyMaintenance::open(KeyMaintenanceConfig {
        database_url: None,
        db_path: path.to_string_lossy().into_owned(),
        keyring: keyring.clone(),
    })
    .unwrap();

    let status = maintenance.status().unwrap();
    assert_eq!(status.active_version, 2);
    assert_eq!(status.configured_versions, vec![1, 2]);
    assert_eq!(status.secret_versions.len(), 1);
    assert_eq!(status.secret_versions[0].key_version, 1);
    assert_eq!(status.secret_versions[0].count, 1);
    assert_eq!(status.approval_dependencies.len(), 1);
    assert_eq!(status.approval_dependencies[0].key_version, 1);
    assert_eq!(status.approval_dependencies[0].pending, 1);
    assert_eq!(status.approval_dependencies[0].approved, 0);
    assert_eq!(status.approval_dependencies[0].total, 1);
    assert!(!status.retirement[0].safe_to_retire);

    let dry_run = maintenance.rewrap(1, false).unwrap();
    assert!(dry_run.rewrap.dry_run);
    assert_eq!(dry_run.rewrap.candidates, 1);
    assert_eq!(dry_run.rewrap.rewrapped, 0);
    assert_eq!(dry_run.retirement.encrypted_secrets, 1);

    let still_v1 = SecretStore::open_with_keyring(&path, keyring.clone()).unwrap();
    assert_eq!(
        still_v1
            .metadata(workspace, "provider-token")
            .unwrap()
            .unwrap()
            .key_version,
        1
    );
    drop(still_v1);

    let applied = maintenance.rewrap(1, true).unwrap();
    assert!(!applied.rewrap.dry_run);
    assert_eq!(applied.rewrap.candidates, 1);
    assert_eq!(applied.rewrap.rewrapped, 1);
    assert_eq!(applied.rewrap.skipped_concurrent, 0);
    assert_eq!(applied.retirement.encrypted_secrets, 0);
    assert_eq!(applied.retirement.actionable_approvals, 1);
    assert!(!applied.retirement.safe_to_retire);

    let rewrapped = SecretStore::open_with_keyring(&path, keyring.clone()).unwrap();
    let metadata = rewrapped
        .metadata(workspace, "provider-token")
        .unwrap()
        .unwrap();
    assert_eq!(metadata.key_version, 2);
    assert_eq!(metadata.reference.id, original.id);
    assert_eq!(
        metadata.reference.allowed_origin.as_deref(),
        Some("https://api.example.com")
    );
    assert_eq!(
        rewrapped.resolve(workspace, "provider-token").unwrap(),
        "hidden-value"
    );
    drop(rewrapped);

    let rotated_approvals =
        ApprovalStore::open_with_digest_keyring(&path, derive_digest_keyring(&keyring)).unwrap();
    rotated_approvals
        .decide(
            workspace,
            approval.approval_id,
            approver,
            ApprovalDecision::Deny,
        )
        .unwrap();
    drop(rotated_approvals);

    let ready = maintenance.retire_check(1).unwrap();
    assert_eq!(ready.encrypted_secrets, 0);
    assert_eq!(ready.actionable_approvals, 0);
    assert_eq!(ready.malformed_actionable_approvals, 0);
    assert!(ready.safe_to_retire);

    let active = maintenance.retire_check(2).unwrap();
    assert!(active.active);
    assert!(!active.safe_to_retire);

    let _ = fs::remove_file(path);
}

#[test]
fn digest1_implicit_v1_approval_blocks_v1_retirement() {
    let path = std::env::temp_dir().join(format!("hooktry-key-digest1-{}.db", Uuid::now_v7()));
    let workspace = Uuid::now_v7();
    let root_v1 = [0x31; 32];
    let root_v2 = [0x32; 32];

    let approvals = ApprovalStore::open(&path, derive_digest_key(&root_v1)).unwrap();
    let approval = approvals
        .create(workspace, Uuid::now_v7(), &http_request())
        .unwrap();
    drop(approvals);

    let legacy_digest =
        approval
            .request_digest
            .replacen("hmac-sha256:v1:k1:", "hmac-sha256:v1:", 1);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE hosted_approvals SET request_digest=?1 WHERE approval_id=?2",
            rusqlite::params![legacy_digest, approval.approval_id.to_string()],
        )
        .unwrap();
    drop(connection);

    let maintenance = KeyMaintenance::open(KeyMaintenanceConfig {
        database_url: None,
        db_path: path.to_string_lossy().into_owned(),
        keyring: VersionedKeyring::new(2, root_v2, [(1, root_v1)]).unwrap(),
    })
    .unwrap();

    let check = maintenance.retire_check(1).unwrap();
    assert_eq!(check.actionable_approvals, 1);
    assert!(!check.safe_to_retire);

    let _ = fs::remove_file(path);
}

#[test]
fn malformed_keyed_actionable_approval_fails_retirement_closed() {
    let path = std::env::temp_dir().join(format!("hooktry-key-malformed-{}.db", Uuid::now_v7()));
    let workspace = Uuid::now_v7();
    let root_v1 = [0x41; 32];
    let root_v2 = [0x42; 32];

    let approvals = ApprovalStore::open(&path, derive_digest_key(&root_v1)).unwrap();
    let approval = approvals
        .create(workspace, Uuid::now_v7(), &http_request())
        .unwrap();
    drop(approvals);

    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE hosted_approvals SET request_digest=?1 WHERE approval_id=?2",
            rusqlite::params![
                "hmac-sha256:v1:knot-a-version:deadbeef",
                approval.approval_id.to_string()
            ],
        )
        .unwrap();
    drop(connection);

    let maintenance = KeyMaintenance::open(KeyMaintenanceConfig {
        database_url: None,
        db_path: path.to_string_lossy().into_owned(),
        keyring: VersionedKeyring::new(2, root_v2, [(1, root_v1)]).unwrap(),
    })
    .unwrap();

    let check = maintenance.retire_check(1).unwrap();
    assert_eq!(check.actionable_approvals, 0);
    assert_eq!(check.malformed_actionable_approvals, 1);
    assert!(!check.safe_to_retire);

    let _ = fs::remove_file(path);
}

fn http_request() -> HttpExecutionRequest {
    HttpExecutionRequest {
        method: "POST".to_owned(),
        url: "https://api.example.com/v1/run".to_owned(),
        headers: BTreeMap::new(),
        body: Some(json!({"task": "key-maintenance"})),
        secret_headers: BTreeMap::new(),
        capture: Vec::new(),
        timeout_ms: 5000,
    }
}
