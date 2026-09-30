use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, Uri},
    routing::{get, post},
};
use ortyo::{
    approval::ApprovalDecision, execution::HttpExecutionRequest, hosted_client::HostedClient,
};
use serde_json::json;
use tokio::net::TcpListener;
use uuid::Uuid;

#[derive(Clone, Default)]
struct Seen {
    requests: Arc<Mutex<Vec<(String, String)>>>,
}

#[tokio::test]
async fn hosted_client_keeps_execute_and_approve_credentials_separate() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Seen::default();
    let app = Router::new()
        .route("/_ortyo/hosted/approvals", get(record).post(record))
        .route("/_ortyo/hosted/approvals/{id}", get(record))
        .route("/_ortyo/hosted/approvals/{id}/decision", post(record))
        .route("/_ortyo/hosted/approvals/{id}/execute", post(record))
        .route("/_ortyo/hosted/executions/{id}", get(record))
        .with_state(seen.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = HostedClient::from_lookup(format!("http://{addr}"), |key| match key {
        "ORTYO_TOKEN" => Some("execute-token".to_owned()),
        "ORTYO_APPROVER_TOKEN" => Some("approve-token".to_owned()),
        _ => None,
    });
    let request = HttpExecutionRequest {
        method: "GET".to_owned(),
        url: "https://api.example.com/health".to_owned(),
        headers: BTreeMap::new(),
        body: None,
        secret_headers: BTreeMap::new(),
        capture: vec![],
        timeout_ms: 1000,
    };
    let approval_id = Uuid::now_v7();
    let execution_id = Uuid::now_v7();

    client.create_approval(&request).await.unwrap();
    client.approval_inbox().await.unwrap();
    client.get_approval(approval_id).await.unwrap();
    client
        .decide_approval(approval_id, ApprovalDecision::Approve)
        .await
        .unwrap();
    client
        .execute_approved(approval_id, &request)
        .await
        .unwrap();
    client.get_execution(execution_id).await.unwrap();

    let seen = seen.requests.lock().unwrap().clone();
    assert_eq!(
        seen,
        vec![
            (
                "/_ortyo/hosted/approvals".to_owned(),
                "Bearer execute-token".to_owned()
            ),
            (
                "/_ortyo/hosted/approvals".to_owned(),
                "Bearer approve-token".to_owned()
            ),
            (
                format!("/_ortyo/hosted/approvals/{approval_id}"),
                "Bearer execute-token".to_owned()
            ),
            (
                format!("/_ortyo/hosted/approvals/{approval_id}/decision"),
                "Bearer approve-token".to_owned()
            ),
            (
                format!("/_ortyo/hosted/approvals/{approval_id}/execute"),
                "Bearer execute-token".to_owned()
            ),
            (
                format!("/_ortyo/hosted/executions/{execution_id}"),
                "Bearer execute-token".to_owned()
            ),
        ]
    );
}

#[tokio::test]
async fn hosted_client_never_falls_back_to_execute_token_for_decision() {
    let client = HostedClient::from_lookup("http://127.0.0.1:9", |key| match key {
        "ORTYO_TOKEN" => Some("execute-token".to_owned()),
        _ => None,
    });

    let error = client
        .decide_approval(Uuid::now_v7(), ApprovalDecision::Approve)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        "ORTYO_APPROVER_TOKEN is required for approval decisions"
    );
}

#[tokio::test]
async fn approval_inbox_requires_approver_token_without_execute_fallback() {
    let client = HostedClient::from_lookup("http://127.0.0.1:9", |key| match key {
        "ORTYO_TOKEN" => Some("execute-token".to_owned()),
        _ => None,
    });

    let error = client.approval_inbox().await.unwrap_err();
    assert_eq!(
        error,
        "ORTYO_APPROVER_TOKEN is required for approval decisions"
    );
}

#[tokio::test]
async fn approver_only_client_can_inspect_before_deciding() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let seen = Seen::default();
    let app = Router::new()
        .route("/_ortyo/hosted/approvals/{id}", get(record))
        .with_state(seen.clone());
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = HostedClient::from_lookup(format!("http://{addr}"), |key| match key {
        "ORTYO_APPROVER_TOKEN" => Some("approve-token".to_owned()),
        _ => None,
    });
    let approval_id = Uuid::now_v7();

    client.get_approval(approval_id).await.unwrap();

    assert_eq!(
        seen.requests.lock().unwrap().as_slice(),
        &[(
            format!("/_ortyo/hosted/approvals/{approval_id}"),
            "Bearer approve-token".to_owned()
        )]
    );
}

async fn record(
    State(state): State<Seen>,
    headers: HeaderMap,
    uri: Uri,
) -> Json<serde_json::Value> {
    let authorization = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    state
        .requests
        .lock()
        .unwrap()
        .push((uri.path().to_owned(), authorization));
    Json(json!({"ok": true}))
}
