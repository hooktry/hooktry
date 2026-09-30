use std::time::Duration;

use axum::{Router, body::Bytes, http::StatusCode, routing::post};
use ortyo::{
    domain::{ExposureAccess, ExposureMode, ExposureTarget, Origin},
    exposure::CreateExposure,
    http::AppState,
    relay::RelayBroker,
    relay_ingress::{RelayIngressState, relay_ingress_app},
    relay_transport::{run_runtime_connection, serve_connection},
};
use tokio::net::{TcpListener, TcpStream};

#[tokio::test]
async fn public_http_ingress_crosses_tcp_runtime_and_records_evidence() {
    let target = Router::new().route(
        "/stripe",
        post(|headers: axum::http::HeaderMap, body: Bytes| async move {
            assert_eq!(headers.get("stripe-signature").unwrap(), "t=1,v1=proof");
            (
                StatusCode::ACCEPTED,
                [("x-ortyo-target", "stripe")],
                body,
            )
        }),
    );
    let target_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(target_listener, target).await.unwrap();
    });

    let runtime_state = AppState::default();
    let exposure = runtime_state
        .exposures
        .create_with(
            runtime_state.session.id,
            CreateExposure {
                name: "stripe".to_owned(),
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
    tokio::spawn({
        let state = runtime_state.clone();
        async move {
            let stream = TcpStream::connect(relay_addr).await.unwrap();
            run_runtime_connection(stream, exposure.id, state)
                .await
                .unwrap();
        }
    });

    tokio::time::sleep(Duration::from_millis(25)).await;

    let ingress_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ingress_addr = ingress_listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            ingress_listener,
            relay_ingress_app(RelayIngressState::new(broker)),
        )
        .await
        .unwrap();
    });

    let raw_body = br#"{"type":"payment_intent.succeeded"}"#;
    let response = reqwest::Client::new()
        .post(format!(
            "http://{ingress_addr}/e/{}/stripe?delivery=42",
            exposure.id
        ))
        .header("stripe-signature", "t=1,v1=proof")
        .header("x-duplicate", "first")
        .body(raw_body.to_vec())
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.headers()["x-ortyo-target"], "stripe");
    assert_eq!(response.bytes().await.unwrap().as_ref(), raw_body);

    let interactions = runtime_state.store.all();
    assert_eq!(interactions.len(), 1);
    assert_eq!(interactions[0].origin, Origin::Proxied);
    assert_eq!(interactions[0].request["path"], "/stripe");
    assert_eq!(interactions[0].request["query"], "delivery=42");
    assert_eq!(
        interactions[0].request["headers"]["stripe-signature"],
        "t=1,v1=proof"
    );
    assert_eq!(
        interactions[0].request["body"],
        r#"{"type":"payment_intent.succeeded"}"#
    );
}

#[tokio::test]
async fn ingress_returns_service_unavailable_without_runtime() {
    let broker = RelayBroker::default();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            relay_ingress_app(RelayIngressState::new(broker)),
        )
        .await
        .unwrap();
    });

    let response = reqwest::get(format!("http://{addr}/e/{}/health", uuid::Uuid::now_v7()))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn ingress_times_out_when_registered_runtime_never_answers() {
    let broker = RelayBroker::default();
    let exposure_id = uuid::Uuid::now_v7();
    let _runtime = broker.register(exposure_id).await;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = RelayIngressState {
        broker,
        timeout: Duration::from_millis(25),
    };
    tokio::spawn(async move {
        axum::serve(listener, relay_ingress_app(state)).await.unwrap();
    });

    let response = reqwest::get(format!("http://{addr}/e/{exposure_id}/slow"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
}
