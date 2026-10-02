use axum::{body::Body, http::Request};
use http_body_util::BodyExt;
use hooktry::{
    domain::{Interaction, Origin, Recording},
    http::{AppState, app},
};
use tower::ServiceExt;

#[tokio::test]
async fn recording_replays_captured_interactions_with_provenance() {
    let router = app(AppState::default());

    router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/boundary/github/webhook")
                .header(
                    "traceparent",
                    "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01",
                )
                .header("x-correlation-id", "github-pr-42")
                .body(Body::from(r#"{"action":"opened"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    let response = router
        .clone()
        .oneshot(
            Request::post("/_hooktry/recordings")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let recording: Recording = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(recording.interaction_ids.len(), 1);
    let source_id = recording.interaction_ids[0];

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
    let source: Vec<Interaction> = serde_json::from_slice(&bytes).unwrap();
    let source_context = source[0].context.clone();
    let source_sequence = source[0].observed_sequence.unwrap();

    let response = router
        .oneshot(
            Request::post(format!("/_hooktry/recordings/{}/replay", recording.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let replayed: Vec<Interaction> = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].origin, Origin::Replayed);
    assert!(replayed[0].observed_sequence.unwrap() > source_sequence);
    assert_eq!(replayed[0].source_interaction_id, Some(source_id));
    assert_eq!(replayed[0].operation, "POST /github/webhook");
    assert_eq!(replayed[0].request["body"], r#"{"action":"opened"}"#);
    assert_eq!(replayed[0].context, source_context);
    assert_eq!(
        replayed[0].context.correlation.correlation_id.as_deref(),
        Some("github-pr-42")
    );
}
