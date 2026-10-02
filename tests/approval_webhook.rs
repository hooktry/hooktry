use std::{
    collections::BTreeMap,
    fs,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use hooktry::{
    approval_webhook::{
        APPROVAL_WEBHOOK_SECRET_NAME, ensure_webhook_secret, process_one_for_test,
        retry_delay_ms_for_test,
    },
    execution::HttpExecutionRequest,
    hosted::HostedRelayState,
    relay::RelayBroker,
    relay_auth::CapabilityStore,
    secret::SecretStore,
};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use uuid::Uuid;

#[derive(Clone, Default)]
struct Receiver {
    seen: Arc<Mutex<Vec<(HeaderMap, Value)>>>,
}

#[tokio::test]
async fn webhook_delivery_uses_redacted_approval_and_idempotency_key() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let receiver = Receiver::default();
    let app = Router::new()
        .route("/hook/secret-token", post(capture))
        .with_state(receiver.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let workspace_id = Uuid::now_v7();
    let state = HostedRelayState::websocket_only(
        RelayBroker::default(),
        CapabilityStore::default(),
        "http://hooktry.example",
        "control-token",
    );
    ensure_webhook_secret(
        state.executor.secret_store(),
        workspace_id,
        &format!("http://{addr}/hook/secret-token?key=webhook-secret"),
    )
    .await
    .unwrap();

    let approval = state
        .approvals
        .create(
            workspace_id,
            Uuid::now_v7(),
            &HttpExecutionRequest {
                method: "POST".to_owned(),
                url: "https://api.example.com/danger?token=query-secret".to_owned(),
                headers: BTreeMap::from([("x-sensitive".to_owned(), "header-secret".to_owned())]),
                body: Some(json!({"secret":"body-secret"})),
                secret_headers: BTreeMap::new(),
                capture: vec![],
                timeout_ms: 1000,
            },
        )
        .unwrap();

    assert!(process_one_for_test(&state, workspace_id).await.unwrap());

    let seen = receiver.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    let (headers, body) = &seen[0];
    let notification_id = headers
        .get("idempotency-key")
        .unwrap()
        .to_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    assert_eq!(headers["x-hooktry-event"], "approval_requested");
    assert_eq!(body["event"], "approval_requested");
    assert_eq!(body["notification_id"], notification_id.to_string());
    assert_eq!(body["approval_id"], approval.approval_id.to_string());
    assert_eq!(body["summary"]["method"], "POST");
    assert_eq!(body["summary"]["origin"], "https://api.example.com");
    assert_eq!(body["summary"]["path"], "/danger");

    let serialized = serde_json::to_string(body).unwrap();
    for secret in [
        "query-secret",
        "header-secret",
        "body-secret",
        "webhook-secret",
        "secret-token",
    ] {
        assert!(!serialized.contains(secret));
    }

    assert!(
        state
            .approvals
            .list_undelivered_notifications(workspace_id)
            .unwrap()
            .is_empty()
    );
    let delivered = state
        .approvals
        .get_notification(workspace_id, notification_id)
        .unwrap()
        .unwrap();
    assert!(delivered.delivered_at_unix_ms.is_some());
}

#[tokio::test]
async fn webhook_url_is_encrypted_at_rest() {
    let path = std::env::temp_dir().join(format!(
        "hooktry-approval-webhook-secret-{}.db",
        Uuid::now_v7()
    ));
    let workspace_id = Uuid::now_v7();
    let url = "https://hooks.example.com/private/secret-token?key=do-not-store-plain";
    let store = SecretStore::open(&path, [0x37; 32]).unwrap();

    ensure_webhook_secret(&store, workspace_id, url)
        .await
        .unwrap();
    assert_eq!(
        store
            .resolve(workspace_id, APPROVAL_WEBHOOK_SECRET_NAME)
            .unwrap(),
        url
    );
    drop(store);

    let bytes = fs::read(&path).unwrap();
    for secret in ["secret-token", "do-not-store-plain"] {
        assert!(
            !bytes
                .windows(secret.len())
                .any(|window| window == secret.as_bytes())
        );
    }

    let _ = fs::remove_file(path);
}

#[test]
fn webhook_retry_backoff_is_bounded() {
    assert_eq!(retry_delay_ms_for_test(1), 5_000);
    assert_eq!(retry_delay_ms_for_test(2), 10_000);
    assert_eq!(retry_delay_ms_for_test(3), 20_000);
    assert_eq!(retry_delay_ms_for_test(10), 300_000);
}

async fn capture(
    State(receiver): State<Receiver>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> StatusCode {
    receiver.seen.lock().unwrap().push((headers, body));
    StatusCode::NO_CONTENT
}
