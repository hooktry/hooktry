use axum::{
    Json, Router,
    body::{Body, Bytes},
    http::{Request, StatusCode},
    routing::post,
};
use http_body_util::BodyExt;
use hooktry::{
    domain::{Exposure, ExposureState, Interaction, Origin},
    http::{AppState, app},
};
use serde_json::{Value, json};
use tower::ServiceExt;

#[tokio::test]
async fn local_exposure_proxies_http_and_records_interaction_evidence() {
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

    let router = app(AppState::default());

    let response = router
        .clone()
        .oneshot(
            Request::post("/_hooktry/exposures")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name": "stripe", "port": target_port}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let exposure: Exposure = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(exposure.state, ExposureState::Active);
    assert_eq!(exposure.target.port, target_port);
    assert!(exposure.url.ends_with(&format!("/exposed/{}", exposure.id)));

    let response = router
        .clone()
        .oneshot(
            Request::post(format!("/exposed/{}/webhook?delivery=42", exposure.id))
                .header("content-type", "application/json")
                .header("stripe-signature", "t=123,v1=signature")
                .body(Body::from(r#"{"amount":4999}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(response.headers().get("x-target").unwrap(), "received");
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let target_payload: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(target_payload["received"], r#"{"amount":4999}"#);

    let response = router
        .clone()
        .oneshot(
            Request::get("/_hooktry/interactions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let interactions: Vec<Interaction> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(interactions.len(), 1);
    let interaction = &interactions[0];
    assert_eq!(interaction.origin, Origin::Proxied);
    assert_eq!(interaction.operation, "POST /webhook");
    assert_eq!(interaction.request["exposure_id"], exposure.id.to_string());
    assert_eq!(interaction.request["query"], "delivery=42");
    assert_eq!(
        interaction.request["headers"]["stripe-signature"],
        "t=123,v1=signature"
    );
    assert_eq!(interaction.request["body"], r#"{"amount":4999}"#);
    assert_eq!(interaction.response["status"], 202);
    assert_eq!(
        interaction.response["body"],
        r#"{"received":"{\"amount\":4999}"}"#
    );

    let response = router
        .clone()
        .oneshot(
            Request::delete(format!("/_hooktry/exposures/{}", exposure.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let revoked: Exposure = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(revoked.state, ExposureState::Revoked);

    let response = router
        .oneshot(
            Request::post(format!("/exposed/{}/webhook", exposure.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::GONE);
}
