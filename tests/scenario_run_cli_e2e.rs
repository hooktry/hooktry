use std::{process::Command, sync::Arc};

use axum::{Json, Router, body::Bytes, http::StatusCode, routing::post};
use hooktry::{
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

    let manifest_path =
        std::env::temp_dir().join(format!("hooktry-scenario-run-{}.json", Uuid::now_v7()));
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
        std::env::temp_dir().join(format!("hooktry-scenario-usage-{}.jsonl", Uuid::now_v7()));
    let helper = std::env::current_exe().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_hooktry"))
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
        .env("HOOKTRY_TEST_CHILD", "1")
        .env("HOOKTRY_USAGE_LOG", &usage_path)
        .env("HOOKTRY_USAGE_TOKEN", "usage-secret-not-for-child")
        .env_remove("HOOKTRY_USAGE_ENDPOINT")
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

    assert_eq!(usage_raw.lines().count(), 1);
    assert!(!usage_raw.contains("payment.created"));
    assert!(!usage_raw.contains("/webhook"));
    assert!(!usage_raw.contains(&base_url));
    assert!(!usage_raw.contains(&report.scenario_id.to_string()));
    assert!(!usage_raw.contains(&report.run_id.to_string()));
    assert!(!usage_raw.contains(&report.exposure_id.to_string()));
}

#[tokio::test]
async fn scenario_run_child_helper() {
    if std::env::var("HOOKTRY_TEST_CHILD").as_deref() != Ok("1") {
        return;
    }

    for key in [
        "HOOKTRY_BASE_URL",
        "HOOKTRY_SCENARIO_ID",
        "HOOKTRY_SCENARIO_RUN_ID",
        "HOOKTRY_EXPOSURE_ID",
        "HOOKTRY_EXPOSURE_URL",
    ] {
        assert!(!std::env::var(key).unwrap().is_empty());
    }
    for key in [
        "HOOKTRY_USAGE_LOG",
        "HOOKTRY_USAGE_ENDPOINT",
        "HOOKTRY_USAGE_TOKEN",
    ] {
        assert!(std::env::var_os(key).is_none(), "{key} leaked into child");
    }

    let exposure_url = std::env::var("HOOKTRY_EXPOSURE_URL").unwrap();
    let response = reqwest::Client::new()
        .post(format!("{exposure_url}/webhook?delivery=42"))
        .header("content-type", "application/json")
        .body(r#"{"event":"payment.created","amount":4999}"#)
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
}
