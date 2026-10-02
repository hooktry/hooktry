mod common;

use std::time::Duration;

use axum::{Router, body::Bytes, http::StatusCode, routing::post};
use hooktry::{
    domain::{ExposureAccess, ExposureMode, ExposureTarget, Origin},
    exposure::CreateExposure,
    hosted::{HostedRelayState, ProvisionedExposure, hosted_relay_app},
    http::AppState,
    relay::RelayBroker,
    relay_auth::CapabilityStore,
    relay_transport::{run_runtime_connection, serve_listener},
};
use tokio::net::{TcpListener, TcpStream};

#[tokio::test]
async fn provisioned_hosted_exposure_reaches_local_target_through_boundary() {
    let target = Router::new().route(
        "/webhook",
        post(|headers: axum::http::HeaderMap, body: Bytes| async move {
            assert_eq!(headers.get("x-signature").unwrap(), "proof");
            (StatusCode::ACCEPTED, [("x-target", "local")], body)
        }),
    );
    let target_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(target_listener, target).await.unwrap();
    });

    let broker = RelayBroker::default();
    let capabilities = CapabilityStore::default();

    let relay_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = relay_listener.local_addr().unwrap();
    tokio::spawn(serve_listener(
        relay_listener,
        broker.clone(),
        capabilities.clone(),
    ));

    let hosted_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let hosted_addr = hosted_listener.local_addr().unwrap();
    let hosted_state = HostedRelayState::new(
        broker,
        capabilities,
        format!("http://{hosted_addr}"),
        relay_addr.to_string(),
        "test-control-token",
    );
    tokio::spawn(async move {
        axum::serve(hosted_listener, hosted_relay_app(hosted_state))
            .await
            .unwrap();
    });

    let credential = common::issue_full_access_token(&format!("http://{hosted_addr}")).await;
    let provision: ProvisionedExposure = reqwest::Client::new()
        .post(format!("http://{hosted_addr}/_hooktry/hosted/exposures"))
        .bearer_auth(&credential.token)
        .json(&serde_json::json!({
            "name": "github",
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

    assert_eq!(provision.mode, ExposureMode::Relay);
    assert_eq!(provision.access, ExposureAccess::Public);
    assert_eq!(provision.target_port, target_port);
    assert_eq!(
        provision.public_url,
        format!("http://{hosted_addr}/e/{}", provision.exposure_id)
    );

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
                    port: provision.target_port,
                },
                mode: ExposureMode::Relay,
                access: ExposureAccess::Public,
            },
        )
        .unwrap();

    let runtime_stream =
        TcpStream::connect(provision.relay_addr.as_deref().expect("TCP relay address"))
            .await
            .unwrap();
    let runtime_state_for_task = runtime_state.clone();
    let exposure_id = provision.exposure_id;
    let capability = provision.runtime_capability.clone();
    tokio::spawn(async move {
        run_runtime_connection(
            runtime_stream,
            exposure_id,
            &capability,
            runtime_state_for_task,
        )
        .await
        .unwrap();
    });

    tokio::time::sleep(Duration::from_millis(25)).await;

    let raw_body = br#"{"action":"opened"}"#;
    let response = reqwest::Client::new()
        .post(format!("{}/webhook?delivery=hosted", provision.public_url))
        .header("x-signature", "proof")
        .body(raw_body.to_vec())
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.headers()["x-target"], "local");
    assert_eq!(response.bytes().await.unwrap().as_ref(), raw_body);

    let interactions = runtime_state.store.all();
    assert_eq!(interactions.len(), 1);
    assert_eq!(interactions[0].origin, Origin::Proxied);
    assert_eq!(
        interactions[0].request["exposure_id"],
        exposure_id.to_string()
    );
    assert_eq!(interactions[0].request["path"], "/webhook");
    assert_eq!(interactions[0].request["query"], "delivery=hosted");
    assert_eq!(interactions[0].request["headers"]["x-signature"], "proof");
    assert_eq!(interactions[0].request["body"], r#"{"action":"opened"}"#);
}

#[tokio::test]
async fn socket_disconnect_removes_runtime_registration() {
    let broker = RelayBroker::default();
    let capabilities = CapabilityStore::default();
    let exposure_id = uuid::Uuid::now_v7();
    let capability = capabilities
        .issue(exposure_id, Duration::from_secs(60))
        .unwrap();

    let relay_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = relay_listener.local_addr().unwrap();
    tokio::spawn(serve_listener(relay_listener, broker.clone(), capabilities));

    let stream = TcpStream::connect(relay_addr).await.unwrap();
    let state = AppState::default();
    let capability_token = capability.token.clone();
    let task = tokio::spawn(async move {
        run_runtime_connection(stream, exposure_id, &capability_token, state).await
    });

    tokio::time::sleep(Duration::from_millis(25)).await;
    task.abort();

    let mut unavailable = false;
    for _ in 0..20 {
        match broker.ingress(request_for(exposure_id)).await {
            Err(hooktry::relay::RelayError::RuntimeUnavailable) => {
                unavailable = true;
                break;
            }
            _ => tokio::time::sleep(Duration::from_millis(10)).await,
        }
    }

    assert!(
        unavailable,
        "disconnected runtime registration stayed routable"
    );
}

fn request_for(exposure_id: uuid::Uuid) -> hooktry::relay::RelayRequest {
    hooktry::relay::RelayRequest {
        id: uuid::Uuid::now_v7(),
        exposure_id,
        method: "GET".to_owned(),
        path: "/".to_owned(),
        query: None,
        headers: Vec::new(),
        body: Vec::new(),
    }
}
