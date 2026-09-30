use std::collections::BTreeMap;

use ortyo::{
    approval::{ApprovalDecision, ApprovalRecord, ApprovalState},
    execution::{ExecutionError, ExecutionOutcome, HttpExecutionRequest},
    hosted::{ApprovedExecution, HostedRelayState, hosted_relay_app},
    hosted_identity::{ApiScope, IssuedApiCredential, Workspace},
    relay::RelayBroker,
    relay_auth::CapabilityStore,
};
use serde_json::json;
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
async fn hosted_approval_gate_enforces_separation_exact_request_and_one_shot_use() {
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
    let workspace_a = create_workspace(&client, addr, "approval-a").await;
    let workspace_b = create_workspace(&client, addr, "approval-b").await;

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

    let request = HttpExecutionRequest {
        method: "POST".to_owned(),
        url: "http://127.0.0.1:9/danger?token=do-not-show".to_owned(),
        headers: BTreeMap::new(),
        body: Some(json!({"operation":"delete", "secret":"body-do-not-show"})),
        secret_headers: BTreeMap::new(),
        capture: vec![],
        timeout_ms: 1000,
    };

    let approval: ApprovalRecord = client
        .post(format!("http://{addr}/_ortyo/hosted/approvals"))
        .bearer_auth(&agent_a.token)
        .json(&request)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(approval.workspace_id, workspace_a.id);
    assert_eq!(approval.state, ApprovalState::Pending);
    assert_eq!(approval.summary.path, "/danger");
    let approval_json = serde_json::to_string(&approval).unwrap();
    assert!(!approval_json.contains("do-not-show"));
    assert!(!approval_json.contains("body-do-not-show"));

    let visible_to_approver: ApprovalRecord = client
        .get(format!(
            "http://{addr}/_ortyo/hosted/approvals/{}",
            approval.approval_id
        ))
        .bearer_auth(&approver_a.token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(visible_to_approver.approval_id, approval.approval_id);

    let hidden_from_other_workspace = client
        .get(format!(
            "http://{addr}/_ortyo/hosted/approvals/{}",
            approval.approval_id
        ))
        .bearer_auth(&agent_b.token)
        .send()
        .await
        .unwrap();
    assert_eq!(
        hidden_from_other_workspace.status(),
        reqwest::StatusCode::NOT_FOUND
    );

    let agent_cannot_approve = client
        .post(format!(
            "http://{addr}/_ortyo/hosted/approvals/{}/decision",
            approval.approval_id
        ))
        .bearer_auth(&agent_a.token)
        .json(&json!({"decision": ApprovalDecision::Approve}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        agent_cannot_approve.status(),
        reqwest::StatusCode::FORBIDDEN
    );

    let approved: ApprovalRecord = client
        .post(format!(
            "http://{addr}/_ortyo/hosted/approvals/{}/decision",
            approval.approval_id
        ))
        .bearer_auth(&approver_a.token)
        .json(&json!({"decision": ApprovalDecision::Approve}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(approved.state, ApprovalState::Approved);
    assert_eq!(
        approved.decided_by_credential_id,
        Some(approver_a.credential_id)
    );

    let mut changed = request.clone();
    changed.body = Some(json!({"operation":"create"}));
    let mismatch = client
        .post(format!(
            "http://{addr}/_ortyo/hosted/approvals/{}/execute",
            approval.approval_id
        ))
        .bearer_auth(&agent_a.token)
        .json(&changed)
        .send()
        .await
        .unwrap();
    assert_eq!(mismatch.status(), reqwest::StatusCode::CONFLICT);
    let mismatch_body: serde_json::Value = mismatch.json().await.unwrap();
    assert_eq!(mismatch_body["error"]["code"], "approval_request_mismatch");

    let approver_cannot_execute = client
        .post(format!(
            "http://{addr}/_ortyo/hosted/approvals/{}/execute",
            approval.approval_id
        ))
        .bearer_auth(&approver_a.token)
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(
        approver_cannot_execute.status(),
        reqwest::StatusCode::FORBIDDEN
    );

    let proof: ApprovedExecution = client
        .post(format!(
            "http://{addr}/_ortyo/hosted/approvals/{}/execute",
            approval.approval_id
        ))
        .bearer_auth(&agent_a.token)
        .json(&request)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(proof.approval.state, ApprovalState::Consumed);
    assert_eq!(
        proof.approval.consumed_by_credential_id,
        Some(agent_a.credential_id)
    );
    assert_eq!(
        proof.approval.execution_id,
        Some(proof.execution.execution_id)
    );
    assert_eq!(proof.execution.workspace_id, workspace_a.id);
    assert_eq!(
        proof.execution.outcome,
        ExecutionOutcome::Rejected {
            error: ExecutionError::UnsafeDestination
        }
    );

    let replay = client
        .post(format!(
            "http://{addr}/_ortyo/hosted/approvals/{}/execute",
            approval.approval_id
        ))
        .bearer_auth(&agent_a.token)
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(replay.status(), reqwest::StatusCode::CONFLICT);
    let replay_body: serde_json::Value = replay.json().await.unwrap();
    assert_eq!(replay_body["error"]["code"], "approval_consumed");

    let denied: ApprovalRecord = client
        .post(format!("http://{addr}/_ortyo/hosted/approvals"))
        .bearer_auth(&agent_a.token)
        .json(&request)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let denied: ApprovalRecord = client
        .post(format!(
            "http://{addr}/_ortyo/hosted/approvals/{}/decision",
            denied.approval_id
        ))
        .bearer_auth(&approver_a.token)
        .json(&json!({"decision": ApprovalDecision::Deny}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(denied.state, ApprovalState::Denied);

    let denied_execution = client
        .post(format!(
            "http://{addr}/_ortyo/hosted/approvals/{}/execute",
            denied.approval_id
        ))
        .bearer_auth(&agent_a.token)
        .json(&request)
        .send()
        .await
        .unwrap();
    assert_eq!(denied_execution.status(), reqwest::StatusCode::FORBIDDEN);
    let denied_body: serde_json::Value = denied_execution.json().await.unwrap();
    assert_eq!(denied_body["error"]["code"], "approval_denied");
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
