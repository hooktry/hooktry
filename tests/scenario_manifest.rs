use chrono::Utc;
use ortyo::{
    domain::{ScenarioCheckOutcome, ScenarioOutcome},
    scenario::{CreateScenario, ScenarioManifest, outcome_exit_code},
};
use uuid::Uuid;

#[test]
fn portable_manifest_maps_to_canonical_create_scenario_request() {
    let manifest: ScenarioManifest = serde_json::from_str(
        r#"{
            "name": "stripe webhook",
            "target": {"port": 3000},
            "contracts": [
                {
                    "name": "payment accepted",
                    "operation": "POST /webhook",
                    "request": {"body": {"event": "payment.created"}},
                    "response": {"status": 202}
                }
            ]
        }"#,
    )
    .unwrap();

    let request: CreateScenario = manifest.into();

    assert_eq!(request.name, "stripe webhook");
    assert_eq!(request.port, 3000);
    assert_eq!(request.contracts.len(), 1);
    assert_eq!(request.contracts[0].operation, "POST /webhook");
    assert_eq!(
        request.contracts[0].response.as_ref().unwrap()["status"],
        202
    );
}

#[test]
fn scenario_ci_exit_code_distinguishes_pass_from_behavior_failure() {
    let passed = outcome(true);
    let failed = outcome(false);

    assert_eq!(outcome_exit_code(&passed), 0);
    assert_eq!(outcome_exit_code(&failed), 1);
}

fn outcome(passed: bool) -> ScenarioOutcome {
    ScenarioOutcome {
        run_id: Uuid::now_v7(),
        scenario_id: Uuid::now_v7(),
        completed_at: Utc::now(),
        passed,
        recording_id: None,
        replayed_interaction_ids: Vec::new(),
        checks: vec![ScenarioCheckOutcome {
            contract_id: Uuid::now_v7(),
            cardinality: Default::default(),
            candidate_interaction_ids: Vec::new(),
            matched_interaction_ids: Vec::new(),
            assertion_ids: Vec::new(),
            interaction_id: None,
            assertion_id: None,
            passed,
            error: (!passed).then(|| "expected behavior did not occur".to_owned()),
        }],
    }
}
