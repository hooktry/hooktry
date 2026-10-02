use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use hooktry::{
    domain::Interaction,
    http::{AppState, app},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn http_capture_preserves_raw_headers_and_extracts_normalized_context() {
    let router = app(AppState::default());
    let traceparent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

    let response = router
        .clone()
        .oneshot(
            Request::post("/boundary/payments")
                .header("traceparent", traceparent)
                .header("x-request-id", "req-primary")
                .header("request-id", "req-fallback")
                .header("x-correlation-id", "checkout-42")
                .header("x-causation-id", "event-41")
                .header("x-message-id", "message-99")
                .header("idempotency-key", "payment-42")
                .header("authorization", "Bearer secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let response = router
        .oneshot(
            Request::get("/_hooktry/interactions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let interactions: Vec<Interaction> = serde_json::from_slice(&bytes).unwrap();
    let interaction = &interactions[0];

    assert_eq!(interaction.request["headers"]["traceparent"], traceparent);
    assert_eq!(
        interaction.request["headers"]["authorization"],
        "Bearer secret"
    );
    assert_eq!(
        interaction.context.correlation.trace_id.as_deref(),
        Some("4bf92f3577b34da6a3ce929d0e0e4736")
    );
    assert_eq!(
        interaction.context.correlation.parent_span_id.as_deref(),
        Some("00f067aa0ba902b7")
    );
    assert_eq!(
        interaction.context.correlation.request_id.as_deref(),
        Some("req-primary")
    );
    assert_eq!(
        interaction.context.correlation.correlation_id.as_deref(),
        Some("checkout-42")
    );
    assert_eq!(
        interaction.context.correlation.causation_id.as_deref(),
        Some("event-41")
    );
    assert_eq!(
        interaction.context.correlation.message_id.as_deref(),
        Some("message-99")
    );
    assert_eq!(
        interaction.context.correlation.idempotency_key.as_deref(),
        Some("payment-42")
    );
    assert!(interaction.context.attributes.is_empty());
}

#[tokio::test]
async fn invalid_traceparent_remains_raw_evidence_without_normalized_trace_identity() {
    let router = app(AppState::default());
    let invalid = "00-00000000000000000000000000000000-00f067aa0ba902b7-01";

    router
        .clone()
        .oneshot(
            Request::post("/boundary/payments")
                .header("traceparent", invalid)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let response = router
        .oneshot(
            Request::get("/_hooktry/interactions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let interactions: Vec<Interaction> = serde_json::from_slice(&bytes).unwrap();
    let interaction = &interactions[0];

    assert_eq!(interaction.request["headers"]["traceparent"], invalid);
    assert!(interaction.context.correlation.trace_id.is_none());
    assert!(interaction.context.correlation.parent_span_id.is_none());
}

#[test]
fn interaction_without_context_still_deserializes_for_existing_evidence() {
    let value = serde_json::json!({
        "id": uuid::Uuid::now_v7(),
        "session_id": uuid::Uuid::now_v7(),
        "protocol": "http",
        "direction": "inbound",
        "origin": "observed",
        "operation": "POST /legacy",
        "started_at": chrono::Utc::now(),
        "duration_ms": 1,
        "request": {},
        "response": {"status": 200}
    });

    let interaction: Interaction = serde_json::from_value(value).unwrap();

    assert!(interaction.context.is_empty());
}
