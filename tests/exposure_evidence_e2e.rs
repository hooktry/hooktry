use axum::{
    Json, Router,
    body::{Body, Bytes},
    http::{Request, StatusCode},
    routing::post,
};
use http_body_util::BodyExt;
use ortyo::{
    domain::{AssertionResult, Contract, Exposure, Interaction, Origin, Recording},
    http::{AppState, app},
};
use serde::de::DeserializeOwned;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn exposure_evidence_can_be_recorded_replayed_and_asserted_end_to_end() {
    let target = Router::new().route(
        "/webhook",
        post(|body: Bytes| async move {
            (
                StatusCode::ACCEPTED,
                Json(json!({"received": String::from_utf8_lossy(&body)})),
            )
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(listener, target).await.unwrap();
    });

    let router = app(AppState::default());

    let exposure: Exposure = response_json(
        router
            .clone()
            .oneshot(
                Request::post("/_ortyo/exposures")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"name": "agent-webhook", "port": target_port}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;

    let response = router
        .clone()
        .oneshot(
            Request::post(format!("/exposed/{}/webhook?delivery=42", exposure.id))
                .header("content-type", "application/json")
                .header("x-agent-run", "run-123")
                .body(Body::from(r#"{"event":"payment.created","amount":4999}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let interactions: Vec<Interaction> = response_json(
        router
            .clone()
            .oneshot(
                Request::get("/_ortyo/interactions")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(interactions.len(), 1);
    let source = &interactions[0];
    assert_eq!(source.origin, Origin::Proxied);
    assert_eq!(source.operation, "POST /webhook");
    assert_eq!(source.request["query"], "delivery=42");
    assert_eq!(source.request["headers"]["x-agent-run"], "run-123");
    assert_eq!(source.response["status"], 202);

    let recording: Recording = response_json(
        router
            .clone()
            .oneshot(
                Request::post("/_ortyo/recordings")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(recording.interaction_ids, vec![source.id]);

    let replayed: Vec<Interaction> = response_json(
        router
            .clone()
            .oneshot(
                Request::post(format!("/_ortyo/recordings/{}/replay", recording.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(replayed.len(), 1);
    let replay = &replayed[0];
    assert_eq!(replay.origin, Origin::Replayed);
    assert_eq!(replay.source_interaction_id, Some(source.id));
    assert_eq!(replay.request, source.request);
    assert_eq!(replay.response, source.response);

    let contract: Contract = response_json(
        router
            .clone()
            .oneshot(
                Request::post("/_ortyo/contracts")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "agent webhook accepted",
                            "operation": "POST /webhook",
                            "request": {
                                "query": "delivery=42",
                                "body": "{\"event\":\"payment.created\",\"amount\":4999}"
                            },
                            "response": {"status": 202}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;

    let assertion: AssertionResult = response_json(
        router
            .clone()
            .oneshot(
                Request::post(format!(
                    "/_ortyo/contracts/{}/assert/{}",
                    contract.id, replay.id
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert!(assertion.passed);
    assert!(assertion.mismatches.is_empty());
    assert_eq!(assertion.interaction_id, replay.id);

    let persisted: AssertionResult = response_json(
        router
            .oneshot(
                Request::get(format!("/_ortyo/assertions/{}", assertion.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(persisted.id, assertion.id);
    assert!(persisted.passed);
}

async fn response_json<T: DeserializeOwned>(response: axum::response::Response) -> T {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}
