use std::sync::Arc;

use axum::{Json, Router, body::Bytes, http::StatusCode, routing::post};
use ortyo::{
    domain::{Exposure, Scenario, ScenarioOutcome, ScenarioRun},
    exposure::{ExposureService, LocalExposureProvider, RelayExposureProvider},
    http::{AppState, app},
    mcp::handle,
};
use serde_json::{Value, json};

#[tokio::test]
async fn scenario_drives_setup_evidence_replay_assertion_and_cleanup_through_mcp() {
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

    let scenario_value = mcp_call(
        &base_url,
        "scenario_create",
        json!({
            "name": "payment webhook",
            "port": target_port,
            "contracts": [
                {
                    "name": "payment accepted",
                    "operation": "POST /webhook",
                    "request": {
                        "query": "delivery=42",
                        "body": "{\"event\":\"payment.created\",\"amount\":4999}"
                    },
                    "response": {"status": 202}
                }
            ]
        }),
    )
    .await;
    let scenario: Scenario = serde_json::from_value(scenario_value).unwrap();
    assert_eq!(scenario.contract_ids.len(), 1);

    let persisted_scenario = mcp_call(
        &base_url,
        "scenario_get",
        json!({"scenario_id": scenario.id}),
    )
    .await;
    assert_eq!(persisted_scenario["id"], scenario.id.to_string());

    let run_value = mcp_call(
        &base_url,
        "scenario_start",
        json!({"scenario_id": scenario.id}),
    )
    .await;
    let run: ScenarioRun = serde_json::from_value(run_value).unwrap();
    assert!(run.exposure_url.starts_with(&base_url));

    let response = reqwest::Client::new()
        .post(format!("{}{}", run.exposure_url, "/webhook?delivery=42"))
        .header("content-type", "application/json")
        .header("x-agent-run", "scenario-run-123")
        .body(r#"{"event":"payment.created","amount":4999}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let outcome_value = mcp_call(&base_url, "scenario_complete", json!({"run_id": run.id})).await;
    let outcome: ScenarioOutcome = serde_json::from_value(outcome_value.clone()).unwrap();
    assert!(outcome.passed);
    assert!(outcome.recording_id.is_some());
    assert_eq!(outcome.replayed_interaction_ids.len(), 1);
    assert_eq!(outcome.checks.len(), 1);
    assert!(outcome.checks[0].passed);
    assert!(outcome.checks[0].interaction_id.is_some());
    assert!(outcome.checks[0].assertion_id.is_some());

    let persisted_outcome =
        mcp_call(&base_url, "scenario_outcome_get", json!({"run_id": run.id})).await;
    assert_eq!(persisted_outcome, outcome_value);

    let repeated = mcp_call(&base_url, "scenario_complete", json!({"run_id": run.id})).await;
    assert_eq!(repeated, outcome_value);

    let exposure_value = mcp_call(
        &base_url,
        "exposure_get",
        json!({"exposure_id": run.exposure_id}),
    )
    .await;
    let exposure: Exposure = serde_json::from_value(exposure_value).unwrap();
    assert_eq!(exposure.state, ortyo::domain::ExposureState::Revoked);
}

#[tokio::test]
async fn scenario_without_evidence_completes_with_explicit_failure() {
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

    let scenario = mcp_call(
        &base_url,
        "scenario_create",
        json!({
            "name": "missing webhook",
            "port": 6553,
            "contracts": [
                {
                    "name": "webhook arrives",
                    "operation": "POST /webhook",
                    "response": {"status": 202}
                }
            ]
        }),
    )
    .await;
    let run = mcp_call(
        &base_url,
        "scenario_start",
        json!({"scenario_id": scenario["id"]}),
    )
    .await;

    let outcome = mcp_call(&base_url, "scenario_complete", json!({"run_id": run["id"]})).await;

    assert_eq!(outcome["passed"], false);
    assert!(outcome["recording_id"].is_null());
    assert_eq!(outcome["replayed_interaction_ids"], json!([]));
    assert_eq!(outcome["checks"][0]["passed"], false);
    assert_eq!(
        outcome["checks"][0]["error"],
        "no replayed interaction matched operation POST /webhook"
    );
}

async fn mcp_call(base_url: &str, name: &str, arguments: Value) -> Value {
    let response = handle(
        base_url,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments}
        }),
    )
    .await
    .unwrap()
    .unwrap();

    let result = &response["result"];
    assert_eq!(result["isError"], false, "{result}");
    result["structuredContent"].clone()
}
