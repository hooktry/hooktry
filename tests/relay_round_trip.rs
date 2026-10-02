use axum::{Json, Router, body::Bytes, http::StatusCode, routing::post};
use hooktry::{
    domain::{ExposureAccess, ExposureMode, Origin},
    http::{AppState, proxy_relay_request},
    relay::{RelayBroker, RelayError, RelayRequest},
};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn relay_round_trip_reaches_local_target_and_records_evidence() {
    let target = Router::new().route(
        "/webhook",
        post(|headers: axum::http::HeaderMap, body: Bytes| async move {
            assert_eq!(
                headers.get("stripe-signature").unwrap(),
                "t=123,v1=signature"
            );
            (
                StatusCode::ACCEPTED,
                [("x-target", "received")],
                Json(json!({
                    "received": String::from_utf8_lossy(&body)
                })),
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, target).await.unwrap();
    });

    let state = AppState::default();
    let exposure = state
        .exposures
        .create_with(
            state.session.id,
            hooktry::exposure::CreateExposure {
                name: "stripe".to_owned(),
                target: hooktry::domain::ExposureTarget {
                    host: "127.0.0.1".to_owned(),
                    port: target_port,
                },
                mode: ExposureMode::Relay,
                access: ExposureAccess::Public,
            },
        )
        .unwrap();

    assert_eq!(exposure.mode, ExposureMode::Relay);
    assert_eq!(exposure.access, ExposureAccess::Public);
    assert_eq!(
        exposure.url,
        format!("https://relay.hooktry.test/e/{}", exposure.id)
    );

    let broker = RelayBroker::default();
    let mut runtime = broker.register(exposure.id).await;
    let runtime_state = state.clone();

    let runtime_task = tokio::spawn(async move {
        let work = runtime.recv().await.unwrap();
        let response = proxy_relay_request(&runtime_state, &work.request)
            .await
            .unwrap();
        work.complete(response).unwrap();
    });

    let request_id = Uuid::now_v7();
    let response = broker
        .ingress(RelayRequest {
            id: request_id,
            exposure_id: exposure.id,
            method: "POST".to_owned(),
            path: "/webhook".to_owned(),
            query: Some("delivery=42".to_owned()),
            headers: vec![
                ("content-type".to_owned(), "application/json".to_owned()),
                (
                    "stripe-signature".to_owned(),
                    "t=123,v1=signature".to_owned(),
                ),
            ],
            body: br#"{"amount":4999}"#.to_vec(),
        })
        .await
        .unwrap();

    runtime_task.await.unwrap();

    assert_eq!(response.request_id, request_id);
    assert_eq!(response.status, StatusCode::ACCEPTED.as_u16());
    assert!(
        response
            .headers
            .iter()
            .any(|(name, value)| name == "x-target" && value == "received")
    );
    let payload: serde_json::Value = serde_json::from_slice(&response.body).unwrap();
    assert_eq!(payload["received"], r#"{"amount":4999}"#);

    let interactions = state.store.all();
    assert_eq!(interactions.len(), 1);
    let interaction = &interactions[0];
    assert_eq!(interaction.origin, Origin::Proxied);
    assert_eq!(interaction.operation, "POST /webhook");
    assert_eq!(interaction.request["exposure_id"], exposure.id.to_string());
    assert_eq!(
        interaction.request["relay_request_id"],
        request_id.to_string()
    );
    assert_eq!(interaction.request["query"], "delivery=42");
    assert_eq!(
        interaction.request["headers"]["stripe-signature"],
        "t=123,v1=signature"
    );
    assert_eq!(interaction.request["body"], r#"{"amount":4999}"#);
    assert_eq!(interaction.response["status"], 202);
}

#[tokio::test]
async fn relay_reports_unavailable_and_disconnected_runtimes() {
    let broker = RelayBroker::default();
    let unavailable_id = Uuid::now_v7();

    let unavailable = broker
        .ingress(request_for(unavailable_id))
        .await
        .unwrap_err();
    assert_eq!(unavailable, RelayError::RuntimeUnavailable);

    let disconnected_id = Uuid::now_v7();
    let runtime = broker.register(disconnected_id).await;
    drop(runtime);

    let disconnected = broker
        .ingress(request_for(disconnected_id))
        .await
        .unwrap_err();
    assert_eq!(disconnected, RelayError::RuntimeDisconnected);
}

fn request_for(exposure_id: Uuid) -> RelayRequest {
    RelayRequest {
        id: Uuid::now_v7(),
        exposure_id,
        method: "GET".to_owned(),
        path: "/".to_owned(),
        query: None,
        headers: Vec::new(),
        body: Vec::new(),
    }
}
