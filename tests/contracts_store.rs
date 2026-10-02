use hooktry::{
    domain::{AssertionResult, Contract},
    store::InteractionStore,
};
use serde_json::json;
use uuid::Uuid;

#[test]
fn contracts_and_assertions_survive_database_reopen() {
    let path = std::env::temp_dir().join(format!("hooktry-contracts-{}.db", Uuid::now_v7()));
    let contract = Contract {
        id: Uuid::now_v7(),
        name: "stripe payment intent".into(),
        operation: Some(json!("POST /stripe/payment_intents")),
        request: Some(json!({"method": "POST"})),
        response: Some(json!({"status": 200})),
        context: None,
    };
    let assertion = AssertionResult {
        id: Uuid::now_v7(),
        contract_id: contract.id,
        interaction_id: Uuid::now_v7(),
        passed: true,
        mismatches: Vec::new(),
    };

    {
        let store = InteractionStore::open(&path).unwrap();
        store.save_contract(&contract);
        store.save_assertion(&assertion);
    }

    let reopened = InteractionStore::open(&path).unwrap();
    let persisted_contract = reopened.contract(contract.id).unwrap();
    let persisted_assertion = reopened.assertion(assertion.id).unwrap();

    assert_eq!(persisted_contract.id, contract.id);
    assert_eq!(persisted_contract.name, contract.name);
    assert_eq!(persisted_assertion.id, assertion.id);
    assert_eq!(persisted_assertion.contract_id, contract.id);
    assert_eq!(persisted_assertion.interaction_id, assertion.interaction_id);
    assert!(persisted_assertion.passed);

    std::fs::remove_file(path).unwrap();
}
