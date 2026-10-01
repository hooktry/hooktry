use std::{process::Command, sync::Arc};

use axum::{Json, Router, body::Bytes, http::StatusCode, routing::post};
use ortyo::{
    exposure::{ExposureService, LocalExposureProvider, RelayExposureProvider},
    http::{AppState, app},
    scenario_run::ScenarioRunReport,
    usage::ScenarioUsageEvent,
};
use serde_json::json;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn scenario_run_cli_drives_child_process_and_real_exposure_traffic() {
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

    let manifest_path =
        std::env::temp_dir().join(format!("ortyo-scenario-run-{}.json", Uuid::now_v7()));
    std::fs::write(
        &manifest_path,
        json!({
            "name": "cli payment webhook",
            "target": {"port": target_port},
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
        })
        .to_string(),
    )
    .unwrap();

    let usage_path =
        std::env::temp_dir().join(format!("ortyo-scenario-usage-{}.jsonl", Uuid::now_v7()));
    let helper = std::env::current_exe().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ortyo"))
        .arg("--base-url")
        .arg(&base_url)
        .arg("scenario")
        .arg("run")
        .arg(&manifest_path)
        .arg("--")
        .arg(helper)
        .arg("--exact")
        .arg("scenario_run_child_helper")
        .arg("--nocapture")
        .env("ORTYO_TEST_CHILD", "1")
        .env("ORTYO_USAGE_LOG", &usage_path)
        .env_remove("ORTYO_USAGE_ENDPOINT")
        .env_remove("ORTYO_USAGE_TOKEN")
        .output()
        .unwrap();

    std::fs::remove_file(manifest_path).unwrap();

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: ScenarioRunReport = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report.passed);
    assert!(report.command.success);
    assert_eq!(report.command.exit_code, Some(0));
    assert!(report.outcome.passed);
    assert_eq!(report.outcome.checks.len(), 1);
    assert!(report.outcome.checks[0].passed);

    let usage_raw = std::fs::read_to_string(&usage_path).unwrap();
    std::fs::remove_file(usage_path).unwrap();
    let usage: ScenarioUsageEvent = serde_json::from_str(usage_raw.trim()).unwrap();

    assert_eq!(usage.event, "scenario_run_completed");
    assert!(usage.passed);
    assert!(usage.command_success);
    assert!(usage.outcome_passed);
    assert_eq!(usage.check_count, 1);
    assert_eq!(usage.features.contract_count, 1);
    assert!(usage.features.exact_cardinality);
    assert!(!usage.features.ranged_cardinality);
    assert!(!usage.features.ordering);
    assert!(!usage.features.observation_horizon);
    assert!(!usage.features.settle_window);
    assert!(!usage.features.context_match);
    assert!(!usage.features.idempotency_context);
    assert!(!usage.features.duplicate_guard);

    assert!(!usage_raw.contains("payment.created"));
    assert!(!usage_raw.contains("/webhook"));
    assert!(!usage_raw.contains(&base_url));
}

#[tokio::test]
async fn scenario_run_child_helper() {
    if std::env::var("ORTYO_TEST_CHILD").as_deref() != Ok("1") {
        return;
    }

    for key in [
        "ORTYO_BASE_URL",
        "ORTYO_SCENARIO_ID",
        "ORTYO_SCENARIO_RUN_ID",
        "ORTYO_EXPOSURE_ID",
        "ORTYO_EXPOSURE_URL",
    ] {
        assert!(!std::env::var(key).unwrap().is_empty());
    }

    let exposure_url = std::env::var("ORTYO_EXPOSURE_URL").unwrap();
    let response = reqwest::Client::new()
        .post(format!("{exposure_url}/webhook?delivery=42"))
        .header("content-type", "application/json")
        .body(r#"{"event":"payment.created","amount":4999}"#)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
}
