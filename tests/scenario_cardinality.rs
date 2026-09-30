use std::sync::Arc;

use axum::{Json, Router, body::Bytes, http::StatusCode, routing::post};
use ortyo::{
    domain::{AssertionResult, Scenario, ScenarioOutcome, ScenarioRun},
    exposure::{ExposureService, LocalExposureProvider, RelayExposureProvider},
    http::{AppState, app},
};
use serde_json::{Value, json};

#[tokio::test]
async fn exact_cardinality_detects_duplicate_matching_side_effects() {
    let (base_url, target_port) = system().await;
    let run = create_and_start(
        &base_url,
        target_port,
        json!({
            "name": "payment exactly once",
            "operation": "POST /webhook",
            "count": 1,
            "request": {"body": "{\"event\":\"payment.created\"}"},
            "response": {"status": 202}
        }),
    )
    .await;

    send(&run.exposure_url, r#"{"event":"payment.created"}"#).await;
    send(&run.exposure_url, r#"{"event":"payment.created"}"#).await;

    let outcome = complete(&base_url, run.id).await;

    assert!(!outcome.passed);
    let check = &outcome.checks[0];
    assert!(!check.passed);
    assert_eq!(check.candidate_interaction_ids.len(), 2);
    assert_eq!(check.matched_interaction_ids.len(), 2);
    assert_eq!(check.assertion_ids.len(), 2);
    assert!(check.interaction_id.is_none());
    assert!(check.assertion_id.is_none());
    assert_eq!(
        check.error.as_deref(),
        Some("expected exactly 1, observed 2 matching interactions")
    );
}

#[tokio::test]
async fn min_max_cardinality_accepts_matching_range() {
    let (base_url, target_port) = system().await;
    let run = create_and_start(
        &base_url,
        target_port,
        json!({
            "name": "payment retry range",
            "operation": "POST /webhook",
            "min": 1,
            "max": 3,
            "response": {"status": 202}
        }),
    )
    .await;

    send(&run.exposure_url, r#"{"attempt":1}"#).await;
    send(&run.exposure_url, r#"{"attempt":2}"#).await;

    let outcome = complete(&base_url, run.id).await;

    assert!(outcome.passed);
    let check = &outcome.checks[0];
    assert!(check.passed);
    assert_eq!(check.candidate_interaction_ids.len(), 2);
    assert_eq!(check.matched_interaction_ids.len(), 2);
    assert_eq!(check.assertion_ids.len(), 2);
    assert_eq!(check.cardinality.min, Some(1));
    assert_eq!(check.cardinality.max, Some(3));
}

#[tokio::test]
async fn payload_mismatch_keeps_candidate_and_assertion_evidence() {
    let (base_url, target_port) = system().await;
    let run = create_and_start(
        &base_url,
        target_port,
        json!({
            "name": "payment usd",
            "operation": "POST /webhook",
            "count": 1,
            "request": {"body": "{\"currency\":\"usd\"}"},
            "response": {"status": 202}
        }),
    )
    .await;

    send(&run.exposure_url, r#"{"currency":"eur"}"#).await;

    let outcome = complete(&base_url, run.id).await;
    let check = &outcome.checks[0];

    assert!(!outcome.passed);
    assert_eq!(check.candidate_interaction_ids.len(), 1);
    assert!(check.matched_interaction_ids.is_empty());
    assert_eq!(check.assertion_ids.len(), 1);

    let assertion: AssertionResult = get_json(&format!(
        "{base_url}/_ortyo/assertions/{}",
        check.assertion_ids[0]
    ))
    .await;
    assert!(!assertion.passed);
    assert!(!assertion.mismatches.is_empty());
}

#[tokio::test]
async fn invalid_cardinality_is_rejected_before_scenario_persistence() {
    let (base_url, target_port) = system().await;

    let response = reqwest::Client::new()
        .post(format!("{base_url}/_ortyo/scenarios"))
        .json(&json!({
            "name": "invalid cardinality",
            "port": target_port,
            "contracts": [{
                "name": "webhook",
                "operation": "POST /webhook",
                "count": 1,
                "min": 1
            }]
        }))
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

async fn system() -> (String, u16) {
    let target = Router::new().route(
        "/webhook",
        post(|body: Bytes| async move {
            (
                StatusCode::ACCEPTED,
                Json(json!({"received": String::from_utf8_lossy(&body)})),
            )
        }),
    );
    let target_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(target_listener, target).await.unwrap();
    });

    let ortyo_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ortyo_port = ortyo_listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{ortyo_port}");
    let exposures = ExposureService::with_providers(
        Arc::new(LocalExposureProvider::new(&base_url)),
        Arc::new(RelayExposureProvider::new("https://relay.ortyo.test")),
    );
    let state = AppState {
        exposures,
        ..AppState::default()
    };
    tokio::spawn(async move {
        axum::serve(ortyo_listener, app(state)).await.unwrap();
    });

    (base_url, target_port)
}

async fn create_and_start(base_url: &str, port: u16, contract: Value) -> ScenarioRun {
    let client = reqwest::Client::new();
    let scenario: Scenario = client
        .post(format!("{base_url}/_ortyo/scenarios"))
        .json(&json!({
            "name": "cardinality scenario",
            "port": port,
            "contracts": [contract]
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    client
        .post(format!("{base_url}/_ortyo/scenarios/{}/start", scenario.id))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn send(exposure_url: &str, body: &str) {
    let response = reqwest::Client::new()
        .post(format!("{exposure_url}/webhook"))
        .header("content-type", "application/json")
        .body(body.to_owned())
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

async fn complete(base_url: &str, run_id: uuid::Uuid) -> ScenarioOutcome {
    reqwest::Client::new()
        .post(format!("{base_url}/_ortyo/scenario-runs/{run_id}/complete"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn get_json<T: serde::de::DeserializeOwned>(url: &str) -> T {
    reqwest::get(url).await.unwrap().json().await.unwrap()
}
