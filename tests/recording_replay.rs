use axum::{
    body::Body,
    http::Request,
};
use http_body_util::BodyExt;
use ortyo::{
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
                .body(Body::from(r#"{"action":"opened"}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    let response = router
        .clone()
        .oneshot(
            Request::post("/_ortyo/recordings")
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
        .oneshot(
            Request::post(format!("/_ortyo/recordings/{}/replay", recording.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let replayed: Vec<Interaction> = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(replayed.len(), 1);
    assert_eq!(replayed[0].origin, Origin::Replayed);
    assert_eq!(replayed[0].source_interaction_id, Some(source_id));
    assert_eq!(replayed[0].operation, "POST /github/webhook");
    assert_eq!(replayed[0].request["body"], r#"{"action":"opened"}"#);
}
