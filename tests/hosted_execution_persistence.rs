use std::collections::BTreeMap;

use ortyo::{
    execution::{ExecutionError, ExecutionOutcome, HttpExecutionRequest},
    execution_store::{DurableExecutionRecord, DurableExecutionState},
    hosted::{HostedRelayState, hosted_relay_app},
    hosted_identity::{ApiScope, IssuedApiCredential, Workspace},
    relay::RelayBroker,
    relay_auth::CapabilityStore,
};
use serde_json::json;
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
async fn hosted_execute_persists_terminal_rejection_and_returns_queryable_identity() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = HostedRelayState::websocket_only(
        RelayBroker::default(),
        CapabilityStore::default(),
        format!("http://{addr}"),
        "test-control-token",
    );
    tokio::spawn(async move {
        axum::serve(listener, hosted_relay_app(state))
            .await
            .unwrap();
    });

    let client = reqwest::Client::new();
    let workspace = create_workspace(&client, addr, "execution-a").await;
    let credential = issue_credential(
        &client,
        addr,
        workspace.id,
        "executor-a",
        &[ApiScope::RequestsExecute],
    )
    .await;

    let response = client
        .post(format!("http://{addr}/_ortyo/hosted/execute"))
        .bearer_auth(&credential.token)
        .json(&HttpExecutionRequest {
            method: "GET".to_owned(),
            url: "http://127.0.0.1:9/private".to_owned(),
            headers: BTreeMap::new(),
            body: None,
            secret_headers: BTreeMap::new(),
            capture: vec![],
            timeout_ms: 1000,
        })
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), reqwest::StatusCode::FORBIDDEN);
    let error: serde_json::Value = response.json().await.unwrap();
    assert_eq!(error["error"]["code"], "unsafe_destination");
    let execution_id: Uuid = error["error"]["execution_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let durable: DurableExecutionRecord = client
        .get(format!(
            "http://{addr}/_ortyo/hosted/executions/{execution_id}"
        ))
        .bearer_auth(&credential.token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(durable.execution_id, execution_id);
    assert_eq!(durable.workspace_id, workspace.id);
    assert_eq!(durable.state, DurableExecutionState::Completed);
    assert_eq!(
        durable.outcome,
        Some(ExecutionOutcome::Rejected {
            error: ExecutionError::UnsafeDestination
        })
    );
}

async fn create_workspace(
    client: &reqwest::Client,
    addr: std::net::SocketAddr,
    slug: &str,
) -> Workspace {
    client
        .post(format!("http://{addr}/_ortyo/admin/workspaces"))
        .bearer_auth("test-control-token")
        .json(&json!({"slug": slug}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn issue_credential(
    client: &reqwest::Client,
    addr: std::net::SocketAddr,
    workspace_id: Uuid,
    name: &str,
    scopes: &[ApiScope],
) -> IssuedApiCredential {
    client
        .post(format!(
            "http://{addr}/_ortyo/admin/workspaces/{workspace_id}/credentials"
        ))
        .bearer_auth("test-control-token")
        .json(&json!({
            "name": name,
            "scopes": scopes
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}
