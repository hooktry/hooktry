use std::{sync::Arc, time::Instant};

use axum::{Json, Router, body::Bytes, http::StatusCode, routing::post};
use ortyo::{
    domain::{Scenario, ScenarioOutcome, ScenarioRun},
    exposure::{ExposureService, LocalExposureProvider, RelayExposureProvider},
    http::{AppState, app},
};
use serde_json::json;

#[tokio::test]
async fn scenario_waits_for_matching_context_instead_of_only_matching_operation() {
    let (base_url, target_port) = system().await;
    let client = reqwest::Client::new();
    let scenario: Scenario = client
        .post(format!("{base_url}/_ortyo/scenarios"))
        .json(&json!({
            "name": "correlated payment",
            "port": target_port,
            "observation": {"within_ms": 1000, "settle_ms": 80},
            "contracts": [{
                "name": "payment-42 exactly once",
                "operation": "POST /webhook",
                "count": 1,
                "context": {
                    "correlation_id": "checkout-42",
                    "idempotency_key": "payment-42"
                },
                "response": {"status": 202}
            }]
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

    send(
        &run.exposure_url,
        "checkout-42",
        "payment-wrong",
        r#"{"attempt":1}"#,
    )
    .await;

    let exposure_url = run.exposure_url.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(120)).await;
        send(
            &exposure_url,
            "checkout-42",
            "payment-42",
            r#"{"attempt":2}"#,
        )
        .await;
    });

    let started = Instant::now();
    let outcome: ScenarioOutcome = client
        .post(format!(
            "{base_url}/_ortyo/scenario-runs/{}/complete",
            run.id
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    assert!(outcome.passed);
    assert!(started.elapsed() >= std::time::Duration::from_millis(170));

    let check = &outcome.checks[0];
    assert_eq!(check.candidate_interaction_ids.len(), 2);
    assert_eq!(check.matched_interaction_ids.len(), 1);
    assert_eq!(check.assertion_ids.len(), 2);

    let interactions: Vec<serde_json::Value> = client
        .get(format!("{base_url}/_ortyo/interactions"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let replayed = interactions
        .iter()
        .filter(|item| item["origin"] == "replayed")
        .collect::<Vec<_>>();
    assert_eq!(replayed.len(), 2);
    let replayed_keys = replayed
        .iter()
        .map(|item| item["context"]["correlation"]["idempotency_key"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(replayed_keys.contains(&"payment-wrong"));
    assert!(replayed_keys.contains(&"payment-42"));
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

async fn send(
    exposure_url: &str,
    correlation_id: &str,
    idempotency_key: &str,
    body: &str,
) {
    let response = reqwest::Client::new()
        .post(format!("{exposure_url}/webhook"))
        .header("content-type", "application/json")
        .header("x-correlation-id", correlation_id)
        .header("idempotency-key", idempotency_key)
        .body(body.to_owned())
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
}
