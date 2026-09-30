use std::time::Duration;

use axum::{Json, Router, body::Bytes, http::StatusCode, routing::post};
use ortyo::{
    domain::{ExposureAccess, ExposureMode, ExposureTarget, Origin},
    exposure::CreateExposure,
    http::AppState,
    relay::{RelayBroker, RelayRequest},
    relay_transport::{run_runtime_connection, serve_connection},
};
use serde_json::json;
use tokio::net::{TcpListener, TcpStream};
use uuid::Uuid;

#[tokio::test]
async fn real_tcp_relay_round_trip_reaches_boundary_and_target() {
    let target = Router::new().route(
        "/github",
        post(|headers: axum::http::HeaderMap, body: Bytes| async move {
            assert_eq!(headers.get("x-hub-signature-256").unwrap(), "sha256=proof");
            (
                StatusCode::CREATED,
                [("x-target", "github")],
                Json(json!({"body": String::from_utf8_lossy(&body)})),
            )
        }),
    );
    let target_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(target_listener, target).await.unwrap();
    });

    let state = AppState::default();
    let exposure = state
        .exposures
        .create_with(
            state.session.id,
            CreateExposure {
                name: "github".to_owned(),
                target: ExposureTarget {
                    host: "127.0.0.1".to_owned(),
                    port: target_port,
                },
                mode: ExposureMode::Relay,
                access: ExposureAccess::Public,
            },
        )
        .unwrap();

    let broker = RelayBroker::default();
    let relay_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = relay_listener.local_addr().unwrap();

    let server_broker = broker.clone();
    tokio::spawn(async move {
        let (stream, _) = relay_listener.accept().await.unwrap();
        serve_connection(stream, server_broker).await.unwrap();
    });

    let runtime_state = state.clone();
    let runtime_exposure_id = exposure.id;
    tokio::spawn(async move {
        let stream = TcpStream::connect(relay_addr).await.unwrap();
        run_runtime_connection(stream, runtime_exposure_id, runtime_state)
            .await
            .unwrap();
    });

    // Registration crosses the real TCP socket asynchronously.
    tokio::time::sleep(Duration::from_millis(25)).await;

    let request_id = Uuid::now_v7();
    let response = broker
        .ingress(RelayRequest {
            id: request_id,
            exposure_id: exposure.id,
            method: "POST".to_owned(),
            path: "/github".to_owned(),
            query: Some("delivery=network".to_owned()),
            headers: vec![
                ("content-type".to_owned(), "application/json".to_owned()),
                ("x-hub-signature-256".to_owned(), "sha256=proof".to_owned()),
            ],
            body: br#"{"action":"opened"}"#.to_vec(),
        })
        .await
        .unwrap();

    assert_eq!(response.request_id, request_id);
    assert_eq!(response.status, 201);
    assert!(
        response
            .headers
            .iter()
            .any(|(name, value)| name == "x-target" && value == "github")
    );

    let interactions = state.store.all();
    assert_eq!(interactions.len(), 1);
    assert_eq!(interactions[0].origin, Origin::Proxied);
    assert_eq!(
        interactions[0].request["relay_request_id"],
        request_id.to_string()
    );
    assert_eq!(
        interactions[0].request["headers"]["x-hub-signature-256"],
        "sha256=proof"
    );
    assert_eq!(interactions[0].request["body"], r#"{"action":"opened"}"#);
}

#[tokio::test]
async fn one_tcp_runtime_connection_multiplexes_concurrent_requests() {
    let target = Router::new().route(
        "/echo",
        post(|body: Bytes| async move { (StatusCode::OK, body) }),
    );
    let target_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(target_listener, target).await.unwrap();
    });

    let state = AppState::default();
    let exposure = state
        .exposures
        .create_with(
            state.session.id,
            CreateExposure {
                name: "concurrent".to_owned(),
                target: ExposureTarget {
                    host: "127.0.0.1".to_owned(),
                    port: target_port,
                },
                mode: ExposureMode::Relay,
                access: ExposureAccess::Private,
            },
        )
        .unwrap();

    let broker = RelayBroker::default();
    let relay_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let relay_addr = relay_listener.local_addr().unwrap();
    let server_broker = broker.clone();

    tokio::spawn(async move {
        let (stream, _) = relay_listener.accept().await.unwrap();
        serve_connection(stream, server_broker).await.unwrap();
    });
    tokio::spawn({
        let state = state.clone();
        async move {
            let stream = TcpStream::connect(relay_addr).await.unwrap();
            run_runtime_connection(stream, exposure.id, state)
                .await
                .unwrap();
        }
    });

    tokio::time::sleep(Duration::from_millis(25)).await;

    let first = broker.ingress(request(exposure.id, b"one"));
    let second = broker.ingress(request(exposure.id, b"two"));
    let (first, second) = tokio::join!(first, second);

    assert_eq!(first.unwrap().body, b"one");
    assert_eq!(second.unwrap().body, b"two");
}

fn request(exposure_id: Uuid, body: &[u8]) -> RelayRequest {
    RelayRequest {
        id: Uuid::now_v7(),
        exposure_id,
        method: "POST".to_owned(),
        path: "/echo".to_owned(),
        query: None,
        headers: Vec::new(),
        body: body.to_vec(),
    }
}
