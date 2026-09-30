use std::sync::Arc;

use axum::{Json, Router, body::Bytes, http::StatusCode, routing::post};
use ortyo::{
    domain::{Scenario, ScenarioOutcome, ScenarioRun},
    exposure::{ExposureService, LocalExposureProvider, RelayExposureProvider},
    http::{AppState, app},
};
use serde_json::json;

#[tokio::test]
async fn declared_order_passes_when_matching_interactions_follow_manifest_order() {
    let (base_url, target_port) = system().await;
    let run = create_and_start(&base_url, target_port).await;

    send(&run.exposure_url, "/customer").await;
    send(&run.exposure_url, "/subscription").await;

    let outcome = complete(&base_url, run.id).await;

    assert!(outcome.passed);
    assert!(outcome.checks.iter().all(|check| check.passed));

    let order = outcome.order.unwrap();
    assert!(order.passed);
    assert_eq!(order.observed_interaction_ids.len(), 2);
    assert!(order.violations.is_empty());
}

#[tokio::test]
async fn declared_order_fails_when_contracts_pass_but_evidence_arrives_reversed() {
    let (base_url, target_port) = system().await;
    let run = create_and_start(&base_url, target_port).await;

    send(&run.exposure_url, "/subscription").await;
    send(&run.exposure_url, "/customer").await;

    let outcome = complete(&base_url, run.id).await;

    assert!(!outcome.passed);
    assert!(outcome.checks.iter().all(|check| check.passed));

    let order = outcome.order.unwrap();
    assert!(!order.passed);
    assert_eq!(order.observed_interaction_ids.len(), 2);
    assert_eq!(order.violations.len(), 1);

    let violation = &order.violations[0];
    assert_eq!(
        violation.expected_before_contract_id,
        outcome.checks[0].contract_id
    );
    assert_eq!(
        violation.expected_after_contract_id,
        outcome.checks[1].contract_id
    );
    assert_eq!(
        violation.expected_after_interaction_id,
        order.observed_interaction_ids[0]
    );
    assert_eq!(
        violation.expected_before_interaction_id,
        order.observed_interaction_ids[1]
    );
}

#[tokio::test]
async fn scenario_without_ordering_keeps_existing_order_agnostic_behavior() {
    let (base_url, target_port) = system().await;
    let client = reqwest::Client::new();
    let scenario: Scenario = client
        .post(format!("{base_url}/_ortyo/scenarios"))
        .json(&json!({
            "name": "order agnostic",
            "port": target_port,
            "contracts": [
                {"name": "customer", "operation": "POST /customer", "count": 1},
                {"name": "subscription", "operation": "POST /subscription", "count": 1}
            ]
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    let run: ScenarioRun = client
        .post(format!("{base_url}/_ortyo/scenarios/{}/start", scenario.id))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    send(&run.exposure_url, "/subscription").await;
    send(&run.exposure_url, "/customer").await;

    let outcome = complete(&base_url, run.id).await;

    assert!(outcome.passed);
    assert!(outcome.order.is_none());
}

async fn system() -> (String, u16) {
    let target = Router::new()
        .route(
            "/customer",
            post(|body: Bytes| async move {
                (
                    StatusCode::CREATED,
                    Json(json!({"received": String::from_utf8_lossy(&body)})),
                )
            }),
        )
        .route(
            "/subscription",
            post(|body: Bytes| async move {
                (
                    StatusCode::CREATED,
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

async fn create_and_start(base_url: &str, port: u16) -> ScenarioRun {
    let client = reqwest::Client::new();
    let scenario: Scenario = client
        .post(format!("{base_url}/_ortyo/scenarios"))
        .json(&json!({
            "name": "declared order",
            "port": port,
            "ordering": "declared",
            "contracts": [
                {
                    "name": "create customer",
                    "operation": "POST /customer",
                    "count": 1,
                    "response": {"status": 201}
                },
                {
                    "name": "create subscription",
                    "operation": "POST /subscription",
                    "count": 1,
                    "response": {"status": 201}
                }
            ]
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

async fn send(exposure_url: &str, path: &str) {
    let response = reqwest::Client::new()
        .post(format!("{exposure_url}{path}"))
        .header("content-type", "application/json")
        .body("{}")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
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
