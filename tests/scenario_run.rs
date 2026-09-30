use chrono::Utc;
use ortyo::{
    domain::{ScenarioCheckOutcome, ScenarioOutcome, ScenarioRun, ScenarioRunState},
    scenario_run::{environment, exit_code, report},
};
use uuid::Uuid;

#[test]
fn child_environment_exposes_scenario_identity_and_boundary() {
    let run = run();

    let env = environment("http://ortyo.test", &run)
        .into_iter()
        .collect::<std::collections::HashMap<_, _>>();

    assert_eq!(env["ORTYO_BASE_URL"], "http://ortyo.test");
    assert_eq!(env["ORTYO_SCENARIO_ID"], run.scenario_id.to_string());
    assert_eq!(env["ORTYO_SCENARIO_RUN_ID"], run.id.to_string());
    assert_eq!(env["ORTYO_EXPOSURE_ID"], run.exposure_id.to_string());
    assert_eq!(env["ORTYO_EXPOSURE_URL"], run.exposure_url);
}

#[test]
fn scenario_run_requires_both_child_and_contract_success() {
    let run = run();

    let passing = report(&run, vec!["test".into()], Some(0), outcome(&run, true));
    assert!(passing.passed);
    assert_eq!(exit_code(&passing), 0);

    let child_failed = report(&run, vec!["test".into()], Some(17), outcome(&run, true));
    assert!(!child_failed.passed);
    assert_eq!(child_failed.command.exit_code, Some(17));
    assert_eq!(exit_code(&child_failed), 1);

    let contract_failed = report(&run, vec!["test".into()], Some(0), outcome(&run, false));
    assert!(!contract_failed.passed);
    assert_eq!(exit_code(&contract_failed), 1);

    let signalled = report(&run, vec!["test".into()], None, outcome(&run, true));
    assert!(!signalled.passed);
    assert_eq!(exit_code(&signalled), 1);
}

fn run() -> ScenarioRun {
    ScenarioRun {
        id: Uuid::now_v7(),
        scenario_id: Uuid::now_v7(),
        exposure_id: Uuid::now_v7(),
        exposure_url: "http://ortyo.test/exposed/example".into(),
        state: ScenarioRunState::AwaitingEvidence,
        started_at: Utc::now(),
    }
}

fn outcome(run: &ScenarioRun, passed: bool) -> ScenarioOutcome {
    ScenarioOutcome {
        run_id: run.id,
        scenario_id: run.scenario_id,
        completed_at: Utc::now(),
        passed,
        observation: Default::default(),
        observation_elapsed_ms: 0,
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
            error: (!passed).then(|| "behavior mismatch".to_owned()),
        }],
        order: None,
    }
}
