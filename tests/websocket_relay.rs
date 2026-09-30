use std::time::Duration;

use axum::{Router, body::Bytes, http::StatusCode, routing::post};
use ortyo::{
    domain::{ExposureAccess, ExposureMode, ExposureTarget, Origin},
    exposure::CreateExposure,
    hosted::{HostedRelayState, ProvisionedExposure, hosted_relay_app},
    http::AppState,
    relay::RelayBroker,
    relay_auth::CapabilityStore,
    websocket_transport::run_websocket_runtime,
};
use tokio::net::TcpListener;

#[tokio::test]
async fn websocket_runtime_carries_public_request_to_local_boundary() {
    let target = Router::new().route(
        "/hook",
        post(|headers: axum::http::HeaderMap, body: Bytes| async move {
            assert_eq!(headers.get("x-hook-signature").unwrap(), "signed");
            (StatusCode::CREATED, [("x-runtime", "websocket")], body)
        }),
    );
    let target_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(target_listener, target).await.unwrap();
    });

    let broker = RelayBroker::default();
    let capabilities = CapabilityStore::default();
    let hosted_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let hosted_addr = hosted_listener.local_addr().unwrap();
    let hosted_state = HostedRelayState::new(
        broker,
        capabilities,
        format!("http://{hosted_addr}"),
        "debug-tcp-unused:0",
        "test-control-token",
    );
    tokio::spawn(async move {
        axum::serve(hosted_listener, hosted_relay_app(hosted_state))
            .await
            .unwrap();
    });

    let provision: ProvisionedExposure = reqwest::Client::new()
        .post(format!("http://{hosted_addr}/_ortyo/hosted/exposures"))
        .bearer_auth("test-control-token")
        .json(&serde_json::json!({
            "name": "webhook",
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

    assert_eq!(
        provision.runtime_url,
        format!(
            "ws://{hosted_addr}/_ortyo/runtime/{}",
            provision.exposure_id
        )
    );
    assert!(
        !provision
            .runtime_url
            .contains(&provision.runtime_capability)
    );
    assert!(!provision.public_url.contains(&provision.runtime_capability));

    let runtime_state = AppState::default();
    runtime_state
        .exposures
        .create_with_id(
            runtime_state.session.id,
            provision.exposure_id,
            CreateExposure {
                name: provision.name.clone(),
                target: ExposureTarget {
                    host: "127.0.0.1".to_owned(),
                    port: target_port,
                },
                mode: ExposureMode::Relay,
                access: ExposureAccess::Public,
            },
        )
        .unwrap();

    let runtime_url = provision.runtime_url.clone();
    let capability = provision.runtime_capability.clone();
    let exposure_id = provision.exposure_id;
    let runtime_state_for_task = runtime_state.clone();
    tokio::spawn(async move {
        run_websocket_runtime(
            &runtime_url,
            exposure_id,
            &capability,
            runtime_state_for_task,
        )
        .await
        .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(25)).await;

    let raw_body = br#"{"event":"created"}"#;
    let response = reqwest::Client::new()
        .post(format!("{}/hook?delivery=ws", provision.public_url))
        .header("x-hook-signature", "signed")
        .body(raw_body.to_vec())
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    assert_eq!(response.headers()["x-runtime"], "websocket");
    assert_eq!(response.bytes().await.unwrap().as_ref(), raw_body);

    let interactions = runtime_state.store.all();
    assert_eq!(interactions.len(), 1);
    assert_eq!(interactions[0].origin, Origin::Proxied);
    assert_eq!(
        interactions[0].request["exposure_id"],
        exposure_id.to_string()
    );
    assert_eq!(interactions[0].request["path"], "/hook");
    assert_eq!(interactions[0].request["query"], "delivery=ws");
    assert_eq!(
        interactions[0].request["headers"]["x-hook-signature"],
        "signed"
    );
}

#[tokio::test]
async fn websocket_runtime_rejects_invalid_bearer_capability() {
    let broker = RelayBroker::default();
    let capabilities = CapabilityStore::default();
    let exposure_id = uuid::Uuid::now_v7();
    let valid = capabilities.issue(exposure_id, Duration::from_secs(60));

    let hosted_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let hosted_addr = hosted_listener.local_addr().unwrap();
    let hosted_state = HostedRelayState::new(
        broker,
        capabilities,
        format!("http://{hosted_addr}"),
        "debug-tcp-unused:0",
        "test-control-token",
    );
    tokio::spawn(async move {
        axum::serve(hosted_listener, hosted_relay_app(hosted_state))
            .await
            .unwrap();
    });

    let runtime_url = format!("ws://{hosted_addr}/_ortyo/runtime/{exposure_id}");
    let result = run_websocket_runtime(
        &runtime_url,
        exposure_id,
        &format!("{}-invalid", valid.token),
        AppState::default(),
    )
    .await;

    assert!(result.is_err());
}
