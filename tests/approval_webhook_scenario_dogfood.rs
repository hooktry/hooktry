use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
};
use ortyo::{
    approval_webhook::{ensure_webhook_secret, process_one_for_test},
    domain::{Scenario, ScenarioOutcome, ScenarioRun},
    execution::HttpExecutionRequest,
    exposure::{ExposureService, LocalExposureProvider, RelayExposureProvider},
    hosted::HostedRelayState,
    http::{AppState, app},
    relay::RelayBroker,
    relay_auth::CapabilityStore,
};
use serde_json::{Value, json};
use tokio::net::TcpListener;
use uuid::Uuid;

#[derive(Clone, Default)]
struct Receiver {
    deliveries: Arc<Mutex<Vec<(HeaderMap, Value)>>>,
}

#[tokio::test]
async fn approval_webhook_uses_scenario_as_a_live_exactly_once_idempotency_guard() {
    let workspace_id = Uuid::now_v7();
    let hosted = HostedRelayState::websocket_only(
        RelayBroker::default(),
        CapabilityStore::default(),
        "http://ortyo.example",
        "control-token",
    );

    let approval = hosted
        .approvals
        .create(
            workspace_id,
            Uuid::now_v7(),
            &HttpExecutionRequest {
                method: "POST".to_owned(),
                url: "https://api.example.com/danger".to_owned(),
                headers: Default::default(),
                body: Some(json!({"action":"live-integration-proof"})),
                secret_headers: Default::default(),
                capture: vec![],
                timeout_ms: 1000,
            },
        )
        .unwrap();
    let notification = hosted
        .approvals
        .get_notification_for_approval_async(workspace_id, approval.approval_id)
        .await
        .unwrap()
        .expect("approval notification");

    let receiver = Receiver::default();
    let receiver_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let receiver_port = receiver_listener.local_addr().unwrap().port();
    let receiver_app = Router::new()
        .route("/approval", post(capture))
        .with_state(receiver.clone());
    tokio::spawn(async move {
        axum::serve(receiver_listener, receiver_app).await.unwrap();
    });

    let scenario_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let scenario_addr = scenario_listener.local_addr().unwrap();
    let base_url = format!("http://{scenario_addr}");
    let exposures = ExposureService::with_providers(
        Arc::new(LocalExposureProvider::new(&base_url)),
        Arc::new(RelayExposureProvider::new("https://relay.ortyo.test")),
    );
    tokio::spawn(async move {
        axum::serve(
            scenario_listener,
            app(AppState {
                exposures,
                ..AppState::default()
            }),
        )
        .await
        .unwrap();
    });

    let client = reqwest::Client::new();
    let scenario: Scenario = client
        .post(format!("{base_url}/_ortyo/scenarios"))
        .json(&json!({
            "name": "approval webhook exactly once",
            "port": receiver_port,
            "observation": {
                "within_ms": 1000,
                "settle_ms": 100
            },
            "contracts": [{
                "name": "one approval_requested delivery",
                "operation": "POST /approval",
                "count": 1,
                "context": {
                    "idempotency_key": notification.notification_id.to_string()
                },
                "response": {
                    "status": 204
                }
            }]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let run: ScenarioRun = client
        .post(format!("{base_url}/_ortyo/scenarios/{}/start", scenario.id))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    ensure_webhook_secret(
        hosted.executor.secret_store(),
        workspace_id,
        &format!("{}/approval", run.exposure_url),
    )
    .await
    .unwrap();

    assert!(process_one_for_test(&hosted, workspace_id).await.unwrap());
    assert!(
        !process_one_for_test(&hosted, workspace_id).await.unwrap(),
        "delivered outbox row was claimed twice"
    );

    let outcome: ScenarioOutcome = client
        .post(format!(
            "{base_url}/_ortyo/scenario-runs/{}/complete",
            run.id
        ))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert!(outcome.passed);
    assert_eq!(outcome.checks.len(), 1);
    let check = &outcome.checks[0];
    assert!(check.passed);
    assert_eq!(check.cardinality.count, Some(1));
    assert_eq!(check.candidate_interaction_ids.len(), 1);
    assert_eq!(check.matched_interaction_ids.len(), 1);
    assert_eq!(check.assertion_ids.len(), 1);
    assert_eq!(outcome.observation.within_ms, 1000);
    assert_eq!(outcome.observation.settle_ms, 100);

    let deliveries = receiver.deliveries.lock().unwrap().clone();
    assert_eq!(deliveries.len(), 1);
    let (headers, payload) = &deliveries[0];
    assert_eq!(
        headers["idempotency-key"].to_str().unwrap(),
        notification.notification_id.to_string()
    );
    assert_eq!(headers["x-ortyo-event"], "approval_requested");
    assert_eq!(payload["event"], "approval_requested");
    assert_eq!(
        payload["notification_id"],
        notification.notification_id.to_string()
    );
    assert_eq!(payload["approval_id"], approval.approval_id.to_string());

    let delivered = hosted
        .approvals
        .get_notification_async(workspace_id, notification.notification_id)
        .await
        .unwrap()
        .expect("delivered notification");
    assert!(delivered.delivered_at_unix_ms.is_some());
    assert!(
        hosted
            .approvals
            .list_undelivered_notifications_async(workspace_id)
            .await
            .unwrap()
            .is_empty()
    );

    println!(
        "LIVE-INTEGRATION-PROOF passed: approval outbox -> webhook provider -> Scenario exact-count/idempotency/settle -> one delivery"
    );
}

async fn capture(
    State(receiver): State<Receiver>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> StatusCode {
    receiver.deliveries.lock().unwrap().push((headers, payload));
    StatusCode::NO_CONTENT
}
