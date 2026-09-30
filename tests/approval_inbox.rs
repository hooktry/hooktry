use std::{collections::BTreeMap, time::Duration};

use ortyo::{
    approval::{ApprovalDecision, ApprovalRecord},
    execution::HttpExecutionRequest,
    hosted::{HostedRelayState, hosted_relay_app},
    hosted_identity::{ApiScope, IssuedApiCredential, Workspace},
    relay::RelayBroker,
    relay_auth::CapabilityStore,
};
use serde_json::json;
use tokio::{net::TcpListener, time::sleep};
use uuid::Uuid;

#[tokio::test]
async fn approval_inbox_is_pending_only_approver_only_and_workspace_scoped() {
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
    let workspace_a = create_workspace(&client, addr, "inbox-a").await;
    let workspace_b = create_workspace(&client, addr, "inbox-b").await;
    let agent_a = issue_credential(
        &client,
        addr,
        workspace_a.id,
        "agent-a",
        &[ApiScope::RequestsExecute],
    )
    .await;
    let approver_a = issue_credential(
        &client,
        addr,
        workspace_a.id,
        "approver-a",
        &[ApiScope::RequestsApprove],
    )
    .await;
    let agent_b = issue_credential(
        &client,
        addr,
        workspace_b.id,
        "agent-b",
        &[ApiScope::RequestsExecute],
    )
    .await;
    let approver_b = issue_credential(
        &client,
        addr,
        workspace_b.id,
        "approver-b",
        &[ApiScope::RequestsApprove],
    )
    .await;

    let request = HttpExecutionRequest {
        method: "POST".to_owned(),
        url: "https://api.example.com/actions?token=do-not-show".to_owned(),
        headers: BTreeMap::from([("x-private".to_owned(), "header-do-not-show".to_owned())]),
        body: Some(json!({"secret":"body-do-not-show"})),
        secret_headers: BTreeMap::new(),
        capture: vec![],
        timeout_ms: 1000,
    };

    let first = create_approval(&client, addr, &agent_a, &request).await;
    sleep(Duration::from_millis(2)).await;
    let second = create_approval(&client, addr, &agent_a, &request).await;
    let other_workspace = create_approval(&client, addr, &agent_b, &request).await;

    let execute_only = client
        .get(format!("http://{addr}/_ortyo/hosted/approvals"))
        .bearer_auth(&agent_a.token)
        .send()
        .await
        .unwrap();
    assert_eq!(execute_only.status(), reqwest::StatusCode::FORBIDDEN);

    let inbox_a: Vec<ApprovalRecord> = client
        .get(format!("http://{addr}/_ortyo/hosted/approvals"))
        .bearer_auth(&approver_a.token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(
        inbox_a
            .iter()
            .map(|approval| approval.approval_id)
            .collect::<Vec<_>>(),
        vec![first.approval_id, second.approval_id]
    );
    assert!(
        inbox_a
            .iter()
            .all(|approval| approval.workspace_id == workspace_a.id)
    );
    let inbox_json = serde_json::to_string(&inbox_a).unwrap();
    for secret in ["do-not-show", "header-do-not-show", "body-do-not-show"] {
        assert!(!inbox_json.contains(secret));
    }

    let inbox_b: Vec<ApprovalRecord> = client
        .get(format!("http://{addr}/_ortyo/hosted/approvals"))
        .bearer_auth(&approver_b.token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(inbox_b.len(), 1);
    assert_eq!(inbox_b[0].approval_id, other_workspace.approval_id);

    client
        .post(format!(
            "http://{addr}/_ortyo/hosted/approvals/{}/decision",
            first.approval_id
        ))
        .bearer_auth(&approver_a.token)
        .json(&json!({"decision": ApprovalDecision::Deny}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let after_decision: Vec<ApprovalRecord> = client
        .get(format!("http://{addr}/_ortyo/hosted/approvals"))
        .bearer_auth(&approver_a.token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after_decision.len(), 1);
    assert_eq!(after_decision[0].approval_id, second.approval_id);
}

async fn create_approval(
    client: &reqwest::Client,
    addr: std::net::SocketAddr,
    credential: &IssuedApiCredential,
    request: &HttpExecutionRequest,
) -> ApprovalRecord {
    client
        .post(format!("http://{addr}/_ortyo/hosted/approvals"))
        .bearer_auth(&credential.token)
        .json(request)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
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
