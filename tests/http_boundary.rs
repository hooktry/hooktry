use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use ortyo::http::{AppState, app};
use tower::ServiceExt;

#[tokio::test]
async fn captures_http_request_as_structured_interaction() {
    let state = AppState::default();
    let session_id = state.session.id;
    let router = app(state);

    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/boundary/stripe/payment_intents")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"amount":4999}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let response = router
        .oneshot(
            Request::builder()
                .uri("/_ortyo/interactions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let interactions: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let interaction = &interactions[0];

    assert_eq!(interaction["session_id"], session_id.to_string());
    assert_eq!(interaction["protocol"], "http");
    assert_eq!(interaction["direction"], "inbound");
    assert_eq!(interaction["origin"], "observed");
    assert_eq!(interaction["operation"], "POST /stripe/payment_intents");
    assert_eq!(interaction["request"]["path"], "/stripe/payment_intents");
    assert_eq!(interaction["request"]["body"], r#"{"amount":4999}"#);
    assert_eq!(interaction["response"]["status"], 200);
}
