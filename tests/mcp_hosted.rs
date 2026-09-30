use std::collections::BTreeMap;

use ortyo::{
    execution::HttpExecutionRequest,
    hosted::{HostedRelayState, hosted_relay_app},
    hosted_client::HostedClient,
    hosted_identity::{ApiScope, IssuedApiCredential, Workspace},
    mcp::handle_with_hosted_client,
    relay::RelayBroker,
    relay_auth::CapabilityStore,
};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
async fn mcp_proves_ask_approve_act_and_durable_query_against_hosted_boundary() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");
    let state = HostedRelayState::websocket_only(
        RelayBroker::default(),
        CapabilityStore::default(),
        &base_url,
        "test-control-token",
    );
    tokio::spawn(async move {
        axum::serve(listener, hosted_relay_app(state))
            .await
            .unwrap();
    });

    let http = reqwest::Client::new();
    let workspace = create_workspace(&http, addr, "mcp-control").await;
    let executor = issue_credential(
        &http,
        addr,
        workspace.id,
        "executor",
        &[ApiScope::RequestsExecute],
    )
    .await;
    let approver = issue_credential(
        &http,
        addr,
        workspace.id,
        "approver",
        &[ApiScope::RequestsApprove],
    )
    .await;

    let hosted = HostedClient::from_lookup(&base_url, |key| match key {
        "ORTYO_TOKEN" => Some(executor.token.clone()),
        "ORTYO_APPROVER_TOKEN" => Some(approver.token.clone()),
        _ => None,
    });
    let request = HttpExecutionRequest {
        method: "POST".to_owned(),
        url: "http://127.0.0.1:9/unsafe".to_owned(),
        headers: BTreeMap::new(),
        body: Some(json!({"operation":"prove-mcp"})),
        secret_headers: BTreeMap::new(),
        capture: vec![],
        timeout_ms: 1000,
    };
    let request_json = serde_json::to_value(&request).unwrap();

    let asked = call(
        &base_url,
        &hosted,
        "approval_create",
        json!({"request": request_json.clone()}),
    )
    .await;
    assert_eq!(asked["isError"], false);
    assert_eq!(asked["structuredContent"]["state"], "pending");
    let approval_id = asked["structuredContent"]["approval_id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();

    let inbox = call(&base_url, &hosted, "approval_inbox", json!({})).await;
    assert_eq!(inbox["isError"], false);
    assert_eq!(inbox["structuredContent"].as_array().unwrap().len(), 1);
    assert_eq!(
        inbox["structuredContent"][0]["approval_id"],
        approval_id.to_string()
    );

    let inspected = call(
        &base_url,
        &hosted,
        "approval_get",
        json!({"approval_id": approval_id}),
    )
    .await;
    assert_eq!(inspected["structuredContent"]["state"], "pending");

    let approved = call(
        &base_url,
        &hosted,
        "approval_decide",
        json!({"approval_id": approval_id, "decision": "approve"}),
    )
    .await;
    assert_eq!(approved["isError"], false);
    assert_eq!(approved["structuredContent"]["state"], "approved");

    let after_approval = call(&base_url, &hosted, "approval_inbox", json!({})).await;
    assert_eq!(after_approval["isError"], false);
    assert!(
        after_approval["structuredContent"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let acted = call(
        &base_url,
        &hosted,
        "approval_execute",
        json!({
            "approval_id": approval_id,
            "request": request_json
        }),
    )
    .await;
    assert_eq!(acted["isError"], false);
    assert_eq!(acted["structuredContent"]["approval"]["state"], "consumed");
    assert_eq!(
        acted["structuredContent"]["execution"]["outcome"]["status"],
        "rejected"
    );
    assert_eq!(
        acted["structuredContent"]["execution"]["outcome"]["error"],
        "unsafe_destination"
    );

    let execution_id = acted["structuredContent"]["execution"]["execution_id"]
        .as_str()
        .unwrap()
        .parse::<Uuid>()
        .unwrap();
    assert_eq!(
        acted["structuredContent"]["approval"]["execution_id"],
        execution_id.to_string()
    );

    let durable = call(
        &base_url,
        &hosted,
        "execution_get",
        json!({"execution_id": execution_id}),
    )
    .await;
    assert_eq!(durable["isError"], false);
    assert_eq!(durable["structuredContent"]["state"], "completed");
    assert_eq!(
        durable["structuredContent"]["execution_id"],
        execution_id.to_string()
    );
    assert_eq!(
        durable["structuredContent"]["outcome"],
        acted["structuredContent"]["execution"]["outcome"]
    );
}

async fn call(base_url: &str, hosted: &HostedClient, name: &str, arguments: Value) -> Value {
    handle_with_hosted_client(
        base_url,
        hosted,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments
            }
        }),
    )
    .await
    .unwrap()
    .unwrap()["result"]
        .clone()
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
