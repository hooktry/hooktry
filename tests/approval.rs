use std::{collections::BTreeMap, fs};

use ortyo::{
    approval::{
        ApprovalDecision, ApprovalError, ApprovalState, ApprovalStore, request_digest,
        request_summary,
    },
    execution::{HttpExecutionRequest, SecretCapture, SecretHeaderBinding},
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn approval_summary_is_redacted_and_digest_is_canonical() {
    let mut headers = BTreeMap::new();
    headers.insert("x-api-key".to_owned(), "header-secret".to_owned());

    let mut secret_headers = BTreeMap::new();
    secret_headers.insert(
        "authorization".to_owned(),
        SecretHeaderBinding::SecretRef {
            secret_ref: "ortyo://secrets/provider-token".to_owned(),
            prefix: "Bearer ".to_owned(),
            suffix: String::new(),
        },
    );

    let request = HttpExecutionRequest {
        method: "post".to_owned(),
        url: "https://api.example.com/v1/run?token=query-secret".to_owned(),
        headers,
        body: Some(json!({
            "z": "body-secret",
            "nested": {"b": 2, "a": 1}
        })),
        secret_headers,
        capture: vec![SecretCapture {
            json_pointer: "/credential/token".to_owned(),
            secret_name: "issued-token".to_owned(),
        }],
        timeout_ms: 5000,
    };

    let summary = request_summary(&request).unwrap();
    assert_eq!(summary.method, "POST");
    assert_eq!(summary.origin, "https://api.example.com");
    assert_eq!(summary.path, "/v1/run");
    assert_eq!(summary.header_names, vec!["x-api-key"]);
    assert_eq!(summary.secret_header_names, vec!["authorization"]);
    assert_eq!(summary.capture_names, vec!["issued-token"]);

    let serialized = serde_json::to_string(&summary).unwrap();
    assert!(!serialized.contains("query-secret"));
    assert!(!serialized.contains("header-secret"));
    assert!(!serialized.contains("body-secret"));
    assert!(!serialized.contains("provider-token"));

    let reordered = HttpExecutionRequest {
        body: Some(json!({
            "nested": {"a": 1, "b": 2},
            "z": "body-secret"
        })),
        ..request.clone()
    };
    assert_eq!(
        request_digest(&request).unwrap(),
        request_digest(&reordered).unwrap()
    );
}

#[test]
fn approved_request_is_one_shot_and_survives_sqlite_reopen_without_payloads() {
    let path = std::env::temp_dir().join(format!("ortyo-approval-{}.db", Uuid::now_v7()));
    let workspace_id = Uuid::now_v7();
    let requester_id = Uuid::now_v7();
    let approver_id = Uuid::now_v7();
    let executor_id = Uuid::now_v7();
    let request = http_request(
        "https://api.example.com/v1/run?token=query-secret",
        "body-secret",
    );

    let store = ApprovalStore::open(&path).unwrap();
    let created = store.create(workspace_id, requester_id, &request).unwrap();
    assert_eq!(created.state, ApprovalState::Pending);
    drop(store);

    let bytes = fs::read(&path).unwrap();
    for secret in ["query-secret", "body-secret", "header-secret"] {
        assert!(
            !bytes
                .windows(secret.len())
                .any(|window| window == secret.as_bytes()),
            "approval store persisted redacted input: {secret}"
        );
    }

    let store = ApprovalStore::open(&path).unwrap();
    let approved = store
        .decide(
            workspace_id,
            created.approval_id,
            approver_id,
            ApprovalDecision::Approve,
        )
        .unwrap();
    assert_eq!(approved.state, ApprovalState::Approved);
    assert_eq!(approved.decided_by_credential_id, Some(approver_id));

    let changed = http_request(
        "https://api.example.com/v1/run?token=query-secret",
        "different-body",
    );
    let execution_id = Uuid::now_v7();
    assert_eq!(
        store.consume(
            workspace_id,
            created.approval_id,
            executor_id,
            execution_id,
            &changed
        ),
        Err(ApprovalError::RequestMismatch)
    );
    assert_eq!(
        store
            .get(workspace_id, created.approval_id)
            .unwrap()
            .unwrap()
            .state,
        ApprovalState::Approved
    );

    let consumed = store
        .consume(
            workspace_id,
            created.approval_id,
            executor_id,
            execution_id,
            &request,
        )
        .unwrap();
    assert_eq!(consumed.state, ApprovalState::Consumed);
    assert_eq!(consumed.consumed_by_credential_id, Some(executor_id));
    assert_eq!(consumed.execution_id, Some(execution_id));

    assert_eq!(
        store.consume(
            workspace_id,
            created.approval_id,
            executor_id,
            Uuid::now_v7(),
            &request
        ),
        Err(ApprovalError::Consumed)
    );

    drop(store);
    let reopened = ApprovalStore::open(&path).unwrap();
    assert_eq!(
        reopened
            .get(workspace_id, created.approval_id)
            .unwrap()
            .unwrap()
            .state,
        ApprovalState::Consumed
    );

    let _ = fs::remove_file(path);
}

#[test]
fn denied_and_cross_workspace_approvals_fail_closed() {
    let store = ApprovalStore::default();
    let workspace_id = Uuid::now_v7();
    let other_workspace_id = Uuid::now_v7();
    let request = http_request("https://api.example.com/v1/run", "body");

    let approval = store
        .create(workspace_id, Uuid::now_v7(), &request)
        .unwrap();

    assert_eq!(
        store.get(other_workspace_id, approval.approval_id).unwrap(),
        None
    );
    assert_eq!(
        store.decide(
            other_workspace_id,
            approval.approval_id,
            Uuid::now_v7(),
            ApprovalDecision::Approve
        ),
        Err(ApprovalError::NotFound)
    );

    let denied = store
        .decide(
            workspace_id,
            approval.approval_id,
            Uuid::now_v7(),
            ApprovalDecision::Deny,
        )
        .unwrap();
    assert_eq!(denied.state, ApprovalState::Denied);

    assert_eq!(
        store.consume(
            workspace_id,
            approval.approval_id,
            Uuid::now_v7(),
            Uuid::now_v7(),
            &request
        ),
        Err(ApprovalError::Denied)
    );
    assert_eq!(
        store.decide(
            workspace_id,
            approval.approval_id,
            Uuid::now_v7(),
            ApprovalDecision::Approve
        ),
        Err(ApprovalError::NotPending)
    );
}

fn http_request(url: &str, body: &str) -> HttpExecutionRequest {
    let mut headers = BTreeMap::new();
    headers.insert("x-api-key".to_owned(), "header-secret".to_owned());

    HttpExecutionRequest {
        method: "POST".to_owned(),
        url: url.to_owned(),
        headers,
        body: Some(json!({"payload": body})),
        secret_headers: BTreeMap::new(),
        capture: vec![],
        timeout_ms: 5000,
    }
}
