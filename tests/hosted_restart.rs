use std::{
    fs,
    time::{Duration, UNIX_EPOCH},
};

use axum::{Router, body::Bytes, http::StatusCode, routing::post};
use ortyo::{
    domain::{ExposureAccess, ExposureMode, ExposureTarget},
    exposure::CreateExposure,
    hosted::{HostedRelayState, hosted_relay_app},
    hosted_state::{HostedExposureRecord, HostedExposureStore},
    http::AppState,
    relay::RelayBroker,
    relay_auth::CapabilityStore,
    websocket_transport::run_websocket_runtime,
};
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
async fn hosted_state_survives_restart_and_revocation_survives_next_restart() {
    let db_path = std::env::temp_dir().join(format!("ortyo-hosted-{}.db", Uuid::now_v7()));
    let exposure_id = Uuid::now_v7();

    let first_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let first_addr = first_listener.local_addr().unwrap();
    drop(first_listener);

    let capabilities = CapabilityStore::open(&db_path).unwrap();
    let exposures = HostedExposureStore::open(&db_path).unwrap();
    let capability = capabilities
        .issue(exposure_id, Duration::from_secs(60))
        .unwrap();
    let expires_at = capability
        .expires_at
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let record = HostedExposureRecord {
        exposure_id,
        name: "restart-proof".to_owned(),
        target_port: 1,
        public_url: format!("http://{first_addr}/e/{exposure_id}"),
        runtime_url: format!("ws://{first_addr}/_ortyo/runtime/{exposure_id}"),
        capability_expires_at_unix_seconds: expires_at,
        revoked: false,
    };
    exposures.save(&record).unwrap();

    drop(capabilities);
    drop(exposures);

    let capabilities = CapabilityStore::open(&db_path).unwrap();
    let exposures = HostedExposureStore::open(&db_path).unwrap();
    capabilities
        .authorize(exposure_id, &capability.token)
        .unwrap();
    assert_eq!(exposures.get(exposure_id).unwrap(), Some(record.clone()));

    let bytes = fs::read(&db_path).unwrap();
    assert!(
        !bytes
            .windows(capability.token.len())
            .any(|window| window == capability.token.as_bytes()),
        "raw runtime capability was persisted"
    );

    let target = Router::new().route(
        "/hook",
        post(
            |body: Bytes| async move { (StatusCode::ACCEPTED, [("x-restart", "recovered")], body) },
        ),
    );
    let target_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(target_listener, target).await.unwrap();
    });

    let listener = TcpListener::bind(first_addr).await.unwrap();
    let broker = RelayBroker::default();
    let hosted_state = HostedRelayState::websocket_only_with_store(
        broker,
        capabilities,
        exposures,
        format!("http://{first_addr}"),
        "test-control-token",
    );
    let server = tokio::spawn(async move {
        axum::serve(listener, hosted_relay_app(hosted_state))
            .await
            .unwrap();
    });

    let runtime_state = AppState::default();
    runtime_state
        .exposures
        .adopt_with_url(
            runtime_state.session.id,
            exposure_id,
            CreateExposure {
                name: record.name.clone(),
                target: ExposureTarget {
                    host: "127.0.0.1".to_owned(),
                    port: target_port,
                },
                mode: ExposureMode::Relay,
                access: ExposureAccess::Public,
            },
            record.public_url.clone(),
        )
        .unwrap();

    let runtime_url = record.runtime_url.clone();
    let runtime_capability = capability.token.clone();
    let runtime = tokio::spawn(async move {
        run_websocket_runtime(
            &runtime_url,
            exposure_id,
            &runtime_capability,
            runtime_state,
        )
        .await
    });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let raw_body = br#"{"restart":true}"#;
    let response = reqwest::Client::new()
        .post(format!("{}/hook", record.public_url))
        .body(raw_body.to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.headers()["x-restart"], "recovered");
    assert_eq!(response.bytes().await.unwrap().as_ref(), raw_body);

    let revoke = reqwest::Client::new()
        .delete(http_runtime_url(&record.runtime_url))
        .bearer_auth(&capability.token)
        .send()
        .await
        .unwrap();
    assert_eq!(revoke.status(), StatusCode::NO_CONTENT);

    runtime.abort();
    server.abort();
    let _ = runtime.await;
    let _ = server.await;

    let reopened_capabilities = CapabilityStore::open(&db_path).unwrap();
    let reopened_exposures = HostedExposureStore::open(&db_path).unwrap();
    assert_eq!(
        reopened_capabilities.authorize(exposure_id, &capability.token),
        Err(ortyo::relay_auth::CapabilityError::Revoked)
    );
    assert!(
        reopened_exposures
            .get(exposure_id)
            .unwrap()
            .expect("persisted exposure")
            .revoked
    );

    let second_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let second_addr = second_listener.local_addr().unwrap();
    let second_state = HostedRelayState::websocket_only_with_store(
        RelayBroker::default(),
        reopened_capabilities,
        reopened_exposures,
        format!("http://{second_addr}"),
        "test-control-token",
    );
    let second_server = tokio::spawn(async move {
        axum::serve(second_listener, hosted_relay_app(second_state))
            .await
            .unwrap();
    });

    let reconnect = run_websocket_runtime(
        &format!("ws://{second_addr}/_ortyo/runtime/{exposure_id}"),
        exposure_id,
        &capability.token,
        AppState::default(),
    )
    .await;
    assert!(reconnect.is_err(), "revocation was lost across restart");

    second_server.abort();
    let _ = second_server.await;
    let _ = fs::remove_file(db_path);
}

fn http_runtime_url(runtime_url: &str) -> String {
    runtime_url
        .strip_prefix("ws://")
        .map(|rest| format!("http://{rest}"))
        .or_else(|| {
            runtime_url
                .strip_prefix("wss://")
                .map(|rest| format!("https://{rest}"))
        })
        .unwrap()
}
