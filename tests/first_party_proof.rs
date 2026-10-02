use hooktry::{
    scenario::{CreateScenario, ScenarioManifest},
    usage::ScenarioUsageFeatures,
};

#[test]
fn duplicate_recipe_exercises_idempotency_cardinality_and_settle_proof() {
    let manifest: ScenarioManifest = serde_json::from_str(include_str!(
        "../examples/scenarios/duplicate-idempotency.json"
    ))
    .unwrap();
    let request: CreateScenario = manifest.into();
    let features = ScenarioUsageFeatures::from_request(&request);

    assert_eq!(features.contract_count, 1);
    assert!(features.exact_cardinality);
    assert!(!features.ranged_cardinality);
    assert!(!features.ordering);
    assert!(features.observation_horizon);
    assert!(features.settle_window);
    assert!(features.context_match);
    assert!(features.idempotency_context);
    assert!(features.duplicate_guard);
}

#[test]
fn out_of_order_recipe_exercises_declared_order_and_settle_proof() {
    let manifest: ScenarioManifest =
        serde_json::from_str(include_str!("../examples/scenarios/out-of-order.json")).unwrap();
    let request: CreateScenario = manifest.into();
    let features = ScenarioUsageFeatures::from_request(&request);

    assert_eq!(features.contract_count, 2);
    assert!(features.exact_cardinality);
    assert!(!features.ranged_cardinality);
    assert!(features.ordering);
    assert!(features.observation_horizon);
    assert!(features.settle_window);
    assert!(!features.context_match);
    assert!(!features.idempotency_context);
    assert!(!features.duplicate_guard);
}
