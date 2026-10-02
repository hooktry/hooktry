mod common;

use std::time::Duration;

use axum::{Router, body::Bytes, http::StatusCode, routing::post};
use hooktry::{
    hosted::{HostedRelayState, ProvisionedExposure, hosted_relay_app},
    hosted_runtime::HostedRuntimeStatus,
    http::{AppState, app},
    relay::RelayBroker,
    relay_auth::CapabilityStore,
};
use tokio::net::TcpListener;

#[tokio::test]
async fn daemon_owns_public_runtime_after_provisioning() {
    let target = Router::new().route(
        "/hook",
        post(|body: Bytes| async move { (StatusCode::ACCEPTED, [("x-target", "local")], body) }),
    );
    let target_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(target_listener, target).await.unwrap();
    });

    let hosted_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let hosted_addr = hosted_listener.local_addr().unwrap();
    let hosted_state = HostedRelayState::websocket_only(
        RelayBroker::default(),
        CapabilityStore::default(),
        format!("http://{hosted_addr}"),
        "test-control-token",
    );
    tokio::spawn(async move {
        axum::serve(hosted_listener, hosted_relay_app(hosted_state))
            .await
            .unwrap();
    });

    let local_state = AppState::default();
    let local_state_for_assertions = local_state.clone();
    let local_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = local_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(local_listener, app(local_state)).await.unwrap();
    });

    let client = reqwest::Client::new();
    let credential = common::issue_full_access_token(&format!("http://{hosted_addr}")).await;
    let provision: ProvisionedExposure = client
        .post(format!("http://{hosted_addr}/_hooktry/hosted/exposures"))
        .bearer_auth(&credential.token)
        .json(&serde_json::json!({
            "name": "stripe",
            "target_port": target_port
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let status: HostedRuntimeStatus = client
        .post(format!("http://{local_addr}/_hooktry/hosted-runtimes"))
        .json(&provision)
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(status.exposure_id, provision.exposure_id);
    assert_eq!(status.url, provision.public_url);
    assert!(status.verified);
    assert_eq!(status.runtime_state, "connected");

    let raw_body = br#"{"event":"invoice.paid"}"#;
    let response = client
        .post(format!("{}/hook", status.url))
        .body(raw_body.to_vec())
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.headers()["x-target"], "local");
    assert_eq!(response.bytes().await.unwrap().as_ref(), raw_body);

    let interactions = local_state_for_assertions.store.all();
    assert_eq!(interactions.len(), 1);
    assert_eq!(
        interactions[0].request["exposure_id"],
        provision.exposure_id.to_string()
    );

    client
        .delete(format!(
            "http://{local_addr}/_hooktry/exposures/{}",
            provision.exposure_id
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let mut unavailable = false;
    for _ in 0..30 {
        let response = client
            .get(format!("{}/hook", provision.public_url))
            .send()
            .await
            .unwrap();
        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            unavailable = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    assert!(
        unavailable,
        "revoked exposure left hosted runtime connected"
    );

    let reconnect = hooktry::websocket_transport::run_websocket_runtime(
        &provision.runtime_url,
        provision.exposure_id,
        &provision.runtime_capability,
        AppState::default(),
    )
    .await;
    assert!(
        reconnect.is_err(),
        "revoked runtime capability was accepted again"
    );
}
