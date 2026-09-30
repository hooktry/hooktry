use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use ortyo::{
    domain::{AssertionResult, Contract, Interaction},
    http::{AppState, app},
};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn contract_api_persists_structured_pass_and_fail_assertions() {
    let router = app(AppState::default());

    router
        .clone()
        .oneshot(
            Request::post("/boundary/stripe/payment_intents")
                .body(Body::from(r#"{"amount":4999}"#))
                .unwrap(),
        )
        .await
        .unwrap();

    let response = router
        .clone()
        .oneshot(
            Request::get("/_ortyo/interactions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let interactions: Vec<Interaction> = serde_json::from_slice(&bytes).unwrap();
    let interaction_id = interactions[0].id;

    let response = router
        .clone()
        .oneshot(
            Request::post("/_ortyo/contracts")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "stripe success",
                        "operation": "POST /stripe/payment_intents",
                        "request": {"method": "POST"},
                        "response": {"status": 200}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let contract: Contract = serde_json::from_slice(&bytes).unwrap();

    let response = router
        .clone()
        .oneshot(
            Request::post(format!(
                "/_ortyo/contracts/{}/assert/{}",
                contract.id, interaction_id
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let passed: AssertionResult = serde_json::from_slice(&bytes).unwrap();
    assert!(passed.passed);
    assert!(passed.mismatches.is_empty());

    let response = router
        .clone()
        .oneshot(
            Request::get(format!("/_ortyo/assertions/{}", passed.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let persisted: AssertionResult = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(persisted.id, passed.id);
    assert!(persisted.passed);

    let response = router
        .clone()
        .oneshot(
            Request::post("/_ortyo/contracts")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "stripe created",
                        "response": {"status": 201}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let failing_contract: Contract = serde_json::from_slice(&bytes).unwrap();

    let response = router
        .oneshot(
            Request::post(format!(
                "/_ortyo/contracts/{}/assert/{}",
                failing_contract.id, interaction_id
            ))
            .body(Body::empty())
            .unwrap(),
        )
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let failed: AssertionResult = serde_json::from_slice(&bytes).unwrap();

    assert!(!failed.passed);
    assert_eq!(failed.mismatches.len(), 1);
    assert_eq!(failed.mismatches[0].path, "response");
    assert_eq!(failed.mismatches[0].expected, json!({"status": 201}));
    assert_eq!(failed.mismatches[0].actual, json!({"status": 200}));

}
