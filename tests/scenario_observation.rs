use std::{sync::Arc, time::Instant};

use axum::{Json, Router, body::Bytes, http::StatusCode, routing::post};
use hooktry::{
    domain::{Scenario, ScenarioOutcome, ScenarioRun},
    exposure::{ExposureService, LocalExposureProvider, RelayExposureProvider},
    http::{AppState, app},
};
use serde_json::{Value, json};

#[tokio::test]
async fn waits_for_delayed_expected_interaction_and_settles_before_pass() {
    let (base_url, target_port) = system().await;
    let run = create_and_start(
        &base_url,
        target_port,
        json!({"within_ms": 1000, "settle_ms": 80}),
        json!({
            "name": "delayed webhook",
            "operation": "POST /webhook",
            "count": 1,
            "response": {"status": 202}
        }),
    )
    .await;

    let exposure_url = run.exposure_url.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        send(&exposure_url, r#"{"event":"delayed"}"#).await;
    });

    let started = Instant::now();
    let outcome = complete(&base_url, run.id).await;

    assert!(outcome.passed);
    assert_eq!(outcome.checks[0].matched_interaction_ids.len(), 1);
    assert!(started.elapsed() >= std::time::Duration::from_millis(170));
    assert!(outcome.observation_elapsed_ms >= 170);
    assert!(outcome.observation_elapsed_ms < 1000);
}

#[tokio::test]
async fn duplicate_during_settle_turns_tentative_pass_into_failure() {
    let (base_url, target_port) = system().await;
    let run = create_and_start(
        &base_url,
        target_port,
        json!({"within_ms": 1000, "settle_ms": 200}),
        json!({
            "name": "exactly once webhook",
            "operation": "POST /webhook",
            "count": 1,
            "response": {"status": 202}
        }),
    )
    .await;

    send(&run.exposure_url, r#"{"attempt":1}"#).await;

    let exposure_url = run.exposure_url.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(60)).await;
        send(&exposure_url, r#"{"attempt":2}"#).await;
    });

    let outcome = complete(&base_url, run.id).await;

    assert!(!outcome.passed);
    assert_eq!(outcome.checks[0].matched_interaction_ids.len(), 2);
    assert_eq!(
        outcome.checks[0].error.as_deref(),
        Some("expected exactly 1, observed 2 matching interactions")
    );
    assert!(outcome.observation_elapsed_ms < 1000);
}

#[tokio::test]
async fn zero_cardinality_waits_for_full_observation_window() {
    let (base_url, target_port) = system().await;
    let run = create_and_start(
        &base_url,
        target_port,
        json!({"within_ms": 180, "settle_ms": 50}),
        json!({
            "name": "must not call webhook",
            "operation": "POST /webhook",
            "count": 0
        }),
    )
    .await;

    let started = Instant::now();
    let outcome = complete(&base_url, run.id).await;

    assert!(outcome.passed);
    assert!(started.elapsed() >= std::time::Duration::from_millis(150));
    assert!(outcome.observation_elapsed_ms >= 150);
    assert!(outcome.recording_id.is_none());
}

#[tokio::test]
async fn invalid_observation_policy_is_rejected() {
    let (base_url, target_port) = system().await;
    let client = reqwest::Client::new();

    for observation in [
        json!({"within_ms": 0, "settle_ms": 10}),
        json!({"within_ms": 100, "settle_ms": 101}),
    ] {
        let response = client
            .post(format!("{base_url}/_hooktry/scenarios"))
            .json(&json!({
                "name": "invalid observation",
                "port": target_port,
                "observation": observation,
                "contracts": [{
                    "name": "webhook",
                    "operation": "POST /webhook"
                }]
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
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

    let hooktry_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let hooktry_port = hooktry_listener.local_addr().unwrap().port();
    let base_url = format!("http://127.0.0.1:{hooktry_port}");
    let exposures = ExposureService::with_providers(
        Arc::new(LocalExposureProvider::new(&base_url)),
        Arc::new(RelayExposureProvider::new("https://relay.hooktry.test")),
    );
    let state = AppState {
        exposures,
        ..AppState::default()
    };
    tokio::spawn(async move {
        axum::serve(hooktry_listener, app(state)).await.unwrap();
    });

    (base_url, target_port)
}

async fn create_and_start(
    base_url: &str,
    port: u16,
    observation: Value,
    contract: Value,
) -> ScenarioRun {
    let client = reqwest::Client::new();
    let scenario: Scenario = client
        .post(format!("{base_url}/_hooktry/scenarios"))
        .json(&json!({
            "name": "observation scenario",
            "port": port,
            "observation": observation,
            "contracts": [contract]
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    client
        .post(format!(
            "{base_url}/_hooktry/scenarios/{}/start",
            scenario.id
        ))
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
        .post(format!(
            "{base_url}/_hooktry/scenario-runs/{run_id}/complete"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}
