use chrono::Utc;
use ortyo::{
    domain::{
        Scenario, ScenarioCheckOutcome, ScenarioOrdering, ScenarioOutcome, ScenarioRun,
        ScenarioRunState,
    },
    store::InteractionStore,
};
use uuid::Uuid;

#[test]
fn scenario_run_and_outcome_survive_database_reopen() {
    let path = std::env::temp_dir().join(format!("ortyo-scenario-{}.db", Uuid::now_v7()));
    let scenario = Scenario {
        id: Uuid::now_v7(),
        name: "payment webhook".into(),
        port: 3000,
        contract_ids: vec![Uuid::now_v7()],
        expectations: Vec::new(),
        observation: Default::default(),
        ordering: Some(ScenarioOrdering::Declared),
        created_at: Utc::now(),
    };
    let run = ScenarioRun {
        id: Uuid::now_v7(),
        scenario_id: scenario.id,
        exposure_id: Uuid::now_v7(),
        exposure_url: "http://127.0.0.1:7777/exposed/example".into(),
        state: ScenarioRunState::AwaitingEvidence,
        started_at: Utc::now(),
    };
    let outcome = ScenarioOutcome {
        run_id: run.id,
        scenario_id: scenario.id,
        completed_at: Utc::now(),
        passed: false,
        observation: Default::default(),
        observation_elapsed_ms: 0,
        recording_id: None,
        replayed_interaction_ids: Vec::new(),
        checks: vec![ScenarioCheckOutcome {
            contract_id: scenario.contract_ids[0],
            cardinality: Default::default(),
            candidate_interaction_ids: Vec::new(),
            matched_interaction_ids: Vec::new(),
            assertion_ids: Vec::new(),
            interaction_id: None,
            assertion_id: None,
            passed: false,
            error: Some("no evidence".into()),
        }],
        order: None,
    };

    {
        let store = InteractionStore::open(&path).unwrap();
        store.save_scenario(&scenario);
        store.save_scenario_run(&run);
        store.save_scenario_outcome(&outcome);
    }

    let reopened = InteractionStore::open(&path).unwrap();
    let persisted_scenario = reopened.scenario(scenario.id).unwrap();
    let persisted_run = reopened.scenario_run(run.id).unwrap();
    let persisted_outcome = reopened.scenario_outcome(run.id).unwrap();

    assert_eq!(persisted_scenario.id, scenario.id);
    assert_eq!(persisted_scenario.contract_ids, scenario.contract_ids);
    assert_eq!(persisted_scenario.ordering, Some(ScenarioOrdering::Declared));
    assert_eq!(persisted_run.id, run.id);
    assert_eq!(persisted_run.exposure_id, run.exposure_id);
    assert_eq!(persisted_outcome.run_id, run.id);
    assert!(!persisted_outcome.passed);
    assert_eq!(persisted_outcome.checks.len(), 1);

    std::fs::remove_file(path).unwrap();
}
