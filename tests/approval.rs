use std::{collections::BTreeMap, fs};

use ortyo::{
    approval::{
        ApprovalDecision, ApprovalError, ApprovalNotificationEvent, ApprovalState, ApprovalStore,
        request_digest, request_summary,
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
    let notifications = store.list_undelivered_notifications(workspace_id).unwrap();
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].workspace_id, workspace_id);
    assert_eq!(notifications[0].approval_id, created.approval_id);
    assert_eq!(
        notifications[0].event,
        ApprovalNotificationEvent::ApprovalRequested
    );
    assert_eq!(
        notifications[0].created_at_unix_ms,
        created.requested_at_unix_ms
    );
    assert_eq!(notifications[0].delivered_at_unix_ms, None);
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
fn notification_outbox_survives_reopen_and_delivery_mark_is_idempotent() {
    let path = std::env::temp_dir().join(format!("ortyo-approval-outbox-{}.db", Uuid::now_v7()));
    let workspace_id = Uuid::now_v7();
    let other_workspace_id = Uuid::now_v7();
    let request = http_request("https://api.example.com/v1/run", "body");

    let store = ApprovalStore::open(&path).unwrap();
    let approval = store
        .create(workspace_id, Uuid::now_v7(), &request)
        .unwrap();
    let notification = store
        .list_undelivered_notifications(workspace_id)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    assert_eq!(notification.approval_id, approval.approval_id);
    assert_eq!(
        store
            .get_notification_for_approval(workspace_id, approval.approval_id)
            .unwrap(),
        Some(notification.clone())
    );
    assert_eq!(
        store
            .get_notification_for_approval(other_workspace_id, approval.approval_id)
            .unwrap(),
        None
    );
    assert!(
        store
            .list_undelivered_notifications(other_workspace_id)
            .unwrap()
            .is_empty()
    );
    drop(store);

    let reopened = ApprovalStore::open(&path).unwrap();
    let pending = reopened
        .list_undelivered_notifications(workspace_id)
        .unwrap();
    assert_eq!(pending, vec![notification.clone()]);

    let delivered = reopened
        .mark_notification_delivered(workspace_id, notification.notification_id)
        .unwrap();
    let delivered_at = delivered.delivered_at_unix_ms.unwrap();
    assert!(
        reopened
            .list_undelivered_notifications(workspace_id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        reopened
            .mark_notification_delivered(workspace_id, notification.notification_id)
            .unwrap()
            .delivered_at_unix_ms,
        Some(delivered_at)
    );
    assert_eq!(
        reopened.mark_notification_delivered(other_workspace_id, notification.notification_id),
        Err(ApprovalError::NotFound)
    );

    let _ = fs::remove_file(path);
}

#[test]
fn approval_and_notification_intent_share_one_sqlite_transaction() {
    let path = std::env::temp_dir().join(format!(
        "ortyo-approval-outbox-rollback-{}.db",
        Uuid::now_v7()
    ));
    let workspace_id = Uuid::now_v7();
    let request = http_request("https://api.example.com/v1/run", "body");

    drop(ApprovalStore::open(&path).unwrap());
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER reject_approval_notification
             BEFORE INSERT ON hosted_approval_notification_outbox
             BEGIN
                 SELECT RAISE(ABORT, 'forced outbox failure');
             END;",
        )
        .unwrap();
    drop(connection);

    let store = ApprovalStore::open(&path).unwrap();
    assert!(matches!(
        store.create(workspace_id, Uuid::now_v7(), &request),
        Err(ApprovalError::Storage(_))
    ));
    assert!(store.list_pending(workspace_id).unwrap().is_empty());
    assert!(
        store
            .list_undelivered_notifications(workspace_id)
            .unwrap()
            .is_empty()
    );

    let _ = fs::remove_file(path);
}

#[test]
fn pending_approval_without_notification_is_backfilled_on_reopen() {
    let path = std::env::temp_dir().join(format!(
        "ortyo-approval-outbox-backfill-{}.db",
        Uuid::now_v7()
    ));
    let workspace_id = Uuid::now_v7();
    let request = http_request("https://api.example.com/v1/run", "body");

    let store = ApprovalStore::open(&path).unwrap();
    let approval = store
        .create(workspace_id, Uuid::now_v7(), &request)
        .unwrap();
    drop(store);

    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute(
            "DELETE FROM hosted_approval_notification_outbox WHERE approval_id=?1",
            [approval.approval_id.to_string()],
        )
        .unwrap();
    drop(connection);

    let reopened = ApprovalStore::open(&path).unwrap();
    let notifications = reopened
        .list_undelivered_notifications(workspace_id)
        .unwrap();
    assert_eq!(notifications.len(), 1);
    assert_eq!(notifications[0].approval_id, approval.approval_id);
    assert_eq!(
        notifications[0].created_at_unix_ms,
        approval.requested_at_unix_ms
    );

    let _ = fs::remove_file(path);
}

#[test]
fn notification_claim_is_leased_and_retryable_without_parallel_send() {
    let store = ApprovalStore::default();
    let workspace_id = Uuid::now_v7();
    let request = http_request("https://api.example.com/v1/run", "body");
    let approval = store
        .create(workspace_id, Uuid::now_v7(), &request)
        .unwrap();

    let first = store
        .claim_next_notification(workspace_id, 60_000)
        .unwrap()
        .unwrap();
    assert_eq!(first.record.approval_id, approval.approval_id);
    assert_eq!(first.attempt_count, 1);
    assert!(
        store
            .claim_next_notification(workspace_id, 60_000)
            .unwrap()
            .is_none()
    );

    store
        .fail_notification_claim(
            workspace_id,
            first.record.notification_id,
            first.claim_token,
            0,
            "request_failed",
        )
        .unwrap();

    let second = store
        .claim_next_notification(workspace_id, 60_000)
        .unwrap()
        .unwrap();
    assert_eq!(second.record.notification_id, first.record.notification_id);
    assert_ne!(second.claim_token, first.claim_token);
    assert_eq!(second.attempt_count, 2);

    let delivered = store
        .complete_notification_claim(
            workspace_id,
            second.record.notification_id,
            second.claim_token,
        )
        .unwrap();
    assert!(delivered.delivered_at_unix_ms.is_some());
    assert!(
        store
            .claim_next_notification(workspace_id, 60_000)
            .unwrap()
            .is_none()
    );
}

#[test]
fn deciding_approval_atomically_cancels_undelivered_notification() {
    let store = ApprovalStore::default();
    let workspace_id = Uuid::now_v7();
    let request = http_request("https://api.example.com/v1/run", "body");
    let approval = store
        .create(workspace_id, Uuid::now_v7(), &request)
        .unwrap();
    let claim = store
        .claim_next_notification(workspace_id, 60_000)
        .unwrap()
        .unwrap();

    store
        .decide(
            workspace_id,
            approval.approval_id,
            Uuid::now_v7(),
            ApprovalDecision::Deny,
        )
        .unwrap();

    assert!(
        store
            .list_undelivered_notifications(workspace_id)
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .claim_next_notification(workspace_id, 60_000)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store.complete_notification_claim(
            workspace_id,
            claim.record.notification_id,
            claim.claim_token
        ),
        Err(ApprovalError::NotFound)
    );
}

#[test]
fn pending_inbox_survives_sqlite_reopen_and_excludes_decided_or_other_workspace() {
    let path = std::env::temp_dir().join(format!("ortyo-approval-inbox-{}.db", Uuid::now_v7()));
    let workspace_id = Uuid::now_v7();
    let other_workspace_id = Uuid::now_v7();
    let requester_id = Uuid::now_v7();
    let request = http_request("https://api.example.com/v1/run", "body");

    let store = ApprovalStore::open(&path).unwrap();
    let first = store.create(workspace_id, requester_id, &request).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
    let second = store.create(workspace_id, requester_id, &request).unwrap();
    let other = store
        .create(other_workspace_id, requester_id, &request)
        .unwrap();

    assert_eq!(
        store
            .list_pending(workspace_id)
            .unwrap()
            .iter()
            .map(|approval| approval.approval_id)
            .collect::<Vec<_>>(),
        vec![first.approval_id, second.approval_id]
    );
    assert_eq!(
        store
            .list_pending(other_workspace_id)
            .unwrap()
            .iter()
            .map(|approval| approval.approval_id)
            .collect::<Vec<_>>(),
        vec![other.approval_id]
    );

    store
        .decide(
            workspace_id,
            first.approval_id,
            Uuid::now_v7(),
            ApprovalDecision::Deny,
        )
        .unwrap();
    drop(store);

    let reopened = ApprovalStore::open(&path).unwrap();
    let inbox = reopened.list_pending(workspace_id).unwrap();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].approval_id, second.approval_id);
    assert_eq!(inbox[0].state, ApprovalState::Pending);

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
