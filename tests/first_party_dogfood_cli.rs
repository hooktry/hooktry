use std::{collections::HashSet, process::Command, sync::Arc};

use axum::{Router, http::StatusCode, routing::post};
use ortyo::{
    exposure::{ExposureService, LocalExposureProvider, RelayExposureProvider},
    http::{AppState, app},
    scenario_run::ScenarioRunReport,
    usage::ScenarioUsageEvent,
};
use serde_json::Value;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn temporal_probes_catch_buggy_behavior_and_accept_fixed_behavior() {
    if std::env::var("ORTYO_DOGFOOD_CHILD").as_deref() == Ok("1") {
        return;
    }

    let target = Router::new()
        .route("/webhook", post(|| async { StatusCode::ACCEPTED }))
        .route("/customer", post(|| async { StatusCode::ACCEPTED }))
        .route("/subscription", post(|| async { StatusCode::ACCEPTED }));
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
    tokio::spawn(async move {
        axum::serve(
            ortyo_listener,
            app(AppState {
                exposures,
                ..AppState::default()
            }),
        )
        .await
        .unwrap();
    });

    let temp = std::env::temp_dir().join(format!("ortyo-dogfood-{}", Uuid::now_v7()));
    std::fs::create_dir_all(&temp).unwrap();
    let usage_path = temp.join("usage.jsonl");

    let duplicate_manifest = materialize_recipe(
        include_str!("../examples/scenarios/duplicate-idempotency.json"),
        target_port,
        &temp.join("duplicate.json"),
    );
    let ordering_manifest = materialize_recipe(
        include_str!("../examples/scenarios/out-of-order.json"),
        target_port,
        &temp.join("ordering.json"),
    );

    let duplicate_bug = run_probe(
        &base_url,
        &duplicate_manifest,
        "duplicate_bug",
        &usage_path,
        1,
    );
    assert_behavior_failure(&duplicate_bug, "duplicate bug");

    let duplicate_fixed = run_probe(
        &base_url,
        &duplicate_manifest,
        "duplicate_fixed",
        &usage_path,
        0,
    );
    assert_behavior_pass(&duplicate_fixed, "duplicate fix");

    let order_bug = run_probe(
        &base_url,
        &ordering_manifest,
        "order_bug",
        &usage_path,
        1,
    );
    assert_behavior_failure(&order_bug, "ordering bug");

    let order_fixed = run_probe(
        &base_url,
        &ordering_manifest,
        "order_fixed",
        &usage_path,
        0,
    );
    assert_behavior_pass(&order_fixed, "ordering fix");

    let raw = std::fs::read_to_string(&usage_path).unwrap();
    let events = raw
        .lines()
        .map(|line| serde_json::from_str::<ScenarioUsageEvent>(line).unwrap())
        .collect::<Vec<_>>();

    assert_eq!(events.len(), 4);
    assert_eq!(
        events.iter().map(|event| event.event_id).collect::<HashSet<_>>().len(),
        4
    );

    assert!(!events[0].passed);
    assert!(events[0].command_success);
    assert!(!events[0].outcome_passed);
    assert!(events[0].features.duplicate_guard);
    assert!(events[0].features.idempotency_context);
    assert!(events[0].features.settle_window);
    assert!(!events[0].features.ordering);

    assert!(events[1].passed);
    assert!(events[1].outcome_passed);
    assert!(events[1].features.duplicate_guard);

    assert!(!events[2].passed);
    assert!(events[2].command_success);
    assert!(!events[2].outcome_passed);
    assert!(events[2].features.ordering);
    assert!(events[2].features.settle_window);
    assert!(!events[2].features.duplicate_guard);

    assert!(events[3].passed);
    assert!(events[3].outcome_passed);
    assert!(events[3].features.ordering);

    assert!(!raw.contains("checkout-demo"));
    assert!(!raw.contains("payment-demo"));
    assert!(!raw.contains("/customer"));
    assert!(!raw.contains("/subscription"));
    assert!(!raw.contains("/webhook"));

    println!(
        "DOGFOOD-PROOF passed: duplicate FAIL->PASS, ordering FAIL->PASS, {} privacy-safe events",
        events.len()
    );

    std::fs::remove_dir_all(temp).unwrap();
}

#[tokio::test]
async fn dogfood_child_helper() {
    if std::env::var("ORTYO_DOGFOOD_CHILD").as_deref() != Ok("1") {
        return;
    }

    for key in [
        "ORTYO_USAGE_LOG",
        "ORTYO_USAGE_ENDPOINT",
        "ORTYO_USAGE_TOKEN",
    ] {
        assert!(std::env::var_os(key).is_none(), "{key} leaked into child");
    }

    let exposure_url = std::env::var("ORTYO_EXPOSURE_URL").unwrap();
    let mode = std::env::var("ORTYO_DOGFOOD_MODE").unwrap();

    match mode.as_str() {
        "duplicate_bug" => {
            send_duplicate(&exposure_url).await;
            send_duplicate(&exposure_url).await;
        }
        "duplicate_fixed" => {
            send_duplicate(&exposure_url).await;
        }
        "order_bug" => {
            send(&exposure_url, "/subscription").await;
            send(&exposure_url, "/customer").await;
        }
        "order_fixed" => {
            send(&exposure_url, "/customer").await;
            send(&exposure_url, "/subscription").await;
        }
        other => panic!("unknown dogfood mode: {other}"),
    }
}

fn materialize_recipe(source: &str, port: u16, path: &std::path::Path) -> std::path::PathBuf {
    let mut manifest: Value = serde_json::from_str(source).unwrap();
    manifest["target"]["port"] = Value::from(port);
    std::fs::write(path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    path.to_owned()
}

fn run_probe(
    base_url: &str,
    manifest: &std::path::Path,
    mode: &str,
    usage_path: &std::path::Path,
    expected_exit: i32,
) -> ScenarioRunReport {
    let helper = std::env::current_exe().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_ortyo"))
        .arg("--base-url")
        .arg(base_url)
        .arg("scenario")
        .arg("run")
        .arg(manifest)
        .arg("--")
        .arg(helper)
        .arg("--exact")
        .arg("dogfood_child_helper")
        .arg("--nocapture")
        .env("ORTYO_DOGFOOD_CHILD", "1")
        .env("ORTYO_DOGFOOD_MODE", mode)
        .env("ORTYO_USAGE_LOG", usage_path)
        .env("ORTYO_USAGE_TOKEN", "must-not-reach-child")
        .env_remove("ORTYO_USAGE_ENDPOINT")
        .output()
        .unwrap();

    assert_eq!(
        output.status.code(),
        Some(expected_exit),
        "{mode} stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{mode} did not emit ScenarioRunReport: {error}; stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_behavior_failure(report: &ScenarioRunReport, label: &str) {
    assert!(report.command.success, "{label}: child command failed");
    assert!(!report.outcome.passed, "{label}: behavior unexpectedly passed");
    assert!(!report.passed, "{label}: report unexpectedly passed");
}

fn assert_behavior_pass(report: &ScenarioRunReport, label: &str) {
    assert!(report.command.success, "{label}: child command failed");
    assert!(report.outcome.passed, "{label}: behavior unexpectedly failed");
    assert!(report.passed, "{label}: report unexpectedly failed");
}

async fn send_duplicate(exposure_url: &str) {
    let response = reqwest::Client::new()
        .post(format!("{exposure_url}/webhook"))
        .header("x-correlation-id", "checkout-demo")
        .header("idempotency-key", "payment-demo")
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

async fn send(exposure_url: &str, path: &str) {
    let response = reqwest::Client::new()
        .post(format!("{exposure_url}{path}"))
        .body("{}")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}
