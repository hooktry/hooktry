use std::fs;

use ortyo::{
    hosted::{HostedRelayState, ProvisionedExposure, hosted_relay_app},
    hosted_identity::{ApiScope, HostedIdentityStore, IdentityError, IssuedApiCredential, Workspace},
    http::AppState,
    relay::RelayBroker,
    relay_auth::CapabilityStore,
    websocket_transport::run_websocket_runtime,
};
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
async fn workspace_tokens_enforce_scope_and_tenant_isolation() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = HostedRelayState::websocket_only(
        RelayBroker::default(),
        CapabilityStore::default(),
        format!("http://{addr}"),
        "test-control-token",
    );
    tokio::spawn(async move {
        axum::serve(listener, hosted_relay_app(state)).await.unwrap();
    });

    let client = reqwest::Client::new();
    let workspace_a = create_workspace(&client, addr, "workspace-a").await;
    let workspace_b = create_workspace(&client, addr, "workspace-b").await;

    let token_a = issue_credential(
        &client,
        addr,
        workspace_a.id,
        "full-a",
        &[
            ApiScope::ExposuresCreate,
            ApiScope::ExposuresRead,
            ApiScope::ExposuresRevoke,
        ],
    )
    .await;
    let token_b = issue_credential(
        &client,
        addr,
        workspace_b.id,
        "full-b",
        &[
            ApiScope::ExposuresCreate,
            ApiScope::ExposuresRead,
            ApiScope::ExposuresRevoke,
        ],
    )
    .await;
    let create_only = issue_credential(
        &client,
        addr,
        workspace_a.id,
        "create-only",
        &[ApiScope::ExposuresCreate],
    )
    .await;

    let master_rejected = client
        .post(format!("http://{addr}/_ortyo/hosted/exposures"))
        .bearer_auth("test-control-token")
        .json(&serde_json::json!({"name": "master", "target_port": 3000}))
        .send()
        .await
        .unwrap();
    assert_eq!(master_rejected.status(), reqwest::StatusCode::UNAUTHORIZED);

    let provision: ProvisionedExposure = client
        .post(format!("http://{addr}/_ortyo/hosted/exposures"))
        .bearer_auth(&token_a.token)
        .json(&serde_json::json!({"name": "app", "target_port": 3000}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(provision.workspace_id, Some(workspace_a.id));

    let forbidden = client
        .get(format!(
            "http://{addr}/_ortyo/hosted/exposures/{}",
            provision.exposure_id
        ))
        .bearer_auth(&create_only.token)
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), reqwest::StatusCode::FORBIDDEN);
    let forbidden_body: serde_json::Value = forbidden.json().await.unwrap();
    assert_eq!(forbidden_body["error"]["code"], "forbidden");

    let visible_to_a = client
        .get(format!(
            "http://{addr}/_ortyo/hosted/exposures/{}",
            provision.exposure_id
        ))
        .bearer_auth(&token_a.token)
        .send()
        .await
        .unwrap();
    assert_eq!(visible_to_a.status(), reqwest::StatusCode::OK);

    let hidden_from_b = client
        .get(format!(
            "http://{addr}/_ortyo/hosted/exposures/{}",
            provision.exposure_id
        ))
        .bearer_auth(&token_b.token)
        .send()
        .await
        .unwrap();
    assert_eq!(hidden_from_b.status(), reqwest::StatusCode::NOT_FOUND);

    let cannot_revoke_from_b = client
        .delete(format!(
            "http://{addr}/_ortyo/hosted/exposures/{}",
            provision.exposure_id
        ))
        .bearer_auth(&token_b.token)
        .send()
        .await
        .unwrap();
    assert_eq!(cannot_revoke_from_b.status(), reqwest::StatusCode::NOT_FOUND);

    let list_a: Vec<serde_json::Value> = client
        .get(format!("http://{addr}/_ortyo/hosted/exposures"))
        .bearer_auth(&token_a.token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list_a.len(), 1);

    let list_b: Vec<serde_json::Value> = client
        .get(format!("http://{addr}/_ortyo/hosted/exposures"))
        .bearer_auth(&token_b.token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(list_b.is_empty());

    client
        .delete(format!(
            "http://{addr}/_ortyo/hosted/exposures/{}",
            provision.exposure_id
        ))
        .bearer_auth(&token_a.token)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let reconnect = run_websocket_runtime(
        &provision.runtime_url,
        provision.exposure_id,
        &provision.runtime_capability,
        AppState::default(),
    )
    .await;
    assert!(reconnect.is_err(), "revoked workspace exposure reconnected");
}

#[test]
fn api_credentials_survive_sqlite_reopen_without_persisting_raw_token() {
    let path = std::env::temp_dir().join(format!("ortyo-identity-{}.db", Uuid::now_v7()));

    let store = HostedIdentityStore::open(&path).unwrap();
    let workspace = store.create_workspace("persistent").unwrap();
    let credential = store
        .issue_credential(
            workspace.id,
            "cli",
            &[ApiScope::ExposuresCreate, ApiScope::ExposuresRead],
        )
        .unwrap();
    drop(store);

    let bytes = fs::read(&path).unwrap();
    assert!(
        !bytes
            .windows(credential.token.len())
            .any(|window| window == credential.token.as_bytes()),
        "raw API credential was persisted"
    );

    let reopened = HostedIdentityStore::open(&path).unwrap();
    let authorization = reopened
        .authorize(&credential.token, ApiScope::ExposuresRead)
        .unwrap();
    assert_eq!(authorization.workspace_id, workspace.id);
    assert_eq!(
        reopened.authorize(&credential.token, ApiScope::ExposuresRevoke),
        Err(IdentityError::Forbidden)
    );

    let _ = fs::remove_file(path);
}

async fn create_workspace(
    client: &reqwest::Client,
    addr: std::net::SocketAddr,
    slug: &str,
) -> Workspace {
    client
        .post(format!("http://{addr}/_ortyo/admin/workspaces"))
        .bearer_auth("test-control-token")
        .json(&serde_json::json!({"slug": slug}))
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
        .json(&serde_json::json!({
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
