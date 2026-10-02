use std::fs;

use hooktry::{
    hosted::{HostedRelayState, hosted_relay_app},
    hosted_identity::{ApiScope, HostedIdentityStore, IdentityError},
    relay::RelayBroker,
    relay_auth::CapabilityStore,
};
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
async fn bootstrap_issues_first_token_once_and_token_is_immediately_usable() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = HostedRelayState::websocket_only(
        RelayBroker::default(),
        CapabilityStore::default(),
        format!("http://{addr}"),
        "server-only-control-token",
    );
    tokio::spawn(async move {
        axum::serve(listener, hosted_relay_app(state))
            .await
            .unwrap();
    });

    let client = reqwest::Client::new();
    let bootstrap: serde_json::Value = client
        .post(format!("http://{addr}/_hooktry/bootstrap"))
        .json(&serde_json::json!({"slug": "serhii"}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let token = bootstrap["credential"]["token"].as_str().unwrap();
    assert!(token.starts_with("hooktry_"));
    assert_eq!(bootstrap["workspace"]["slug"], "serhii");
    assert_eq!(bootstrap["credential"]["name"], "initial-cli");

    let provision = client
        .post(format!("http://{addr}/_hooktry/hosted/exposures"))
        .bearer_auth(token)
        .json(&serde_json::json!({"name": "web", "target_port": 3000}))
        .send()
        .await
        .unwrap();
    assert_eq!(provision.status(), reqwest::StatusCode::CREATED);

    let second = client
        .post(format!("http://{addr}/_hooktry/bootstrap"))
        .json(&serde_json::json!({"slug": "attacker"}))
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), reqwest::StatusCode::CONFLICT);
    let body: serde_json::Value = second.json().await.unwrap();
    assert_eq!(body["error"]["code"], "bootstrap_already_completed");
}

#[test]
fn bootstrap_is_durable_and_raw_token_is_not_persisted() {
    let path = std::env::temp_dir().join(format!("hooktry-onboard-{}.db", Uuid::now_v7()));
    let store = HostedIdentityStore::open(&path).unwrap();
    let (workspace, credential) = store
        .bootstrap_first_workspace("serhii", "initial-cli")
        .unwrap();
    drop(store);

    let bytes = fs::read(&path).unwrap();
    assert!(
        !bytes
            .windows(credential.token.len())
            .any(|window| window == credential.token.as_bytes())
    );

    let reopened = HostedIdentityStore::open(&path).unwrap();
    let auth = reopened
        .authorize(&credential.token, ApiScope::ExposuresCreate)
        .unwrap();
    assert_eq!(auth.workspace_id, workspace.id);
    assert_eq!(
        reopened.bootstrap_first_workspace("second", "initial-cli"),
        Err(IdentityError::BootstrapAlreadyCompleted)
    );

    let _ = fs::remove_file(path);
}

#[tokio::test]
async fn concurrent_bootstrap_has_exactly_one_winner() {
    let store = HostedIdentityStore::default();
    let a = {
        let store = store.clone();
        tokio::spawn(async move {
            store
                .bootstrap_first_workspace_async("alpha".to_owned(), "cli".to_owned())
                .await
        })
    };
    let b = {
        let store = store.clone();
        tokio::spawn(async move {
            store
                .bootstrap_first_workspace_async("beta".to_owned(), "cli".to_owned())
                .await
        })
    };

    let results = [a.await.unwrap(), b.await.unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(IdentityError::BootstrapAlreadyCompleted))
            .count(),
        1
    );
}
