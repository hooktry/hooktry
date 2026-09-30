use chrono::Utc;
use ortyo::{
    contract::assert_interaction,
    domain::{Contract, Direction, Interaction, Origin, Protocol},
};
use serde_json::json;
use uuid::Uuid;

fn interaction() -> Interaction {
    Interaction {
        id: Uuid::now_v7(),
        session_id: Uuid::now_v7(),
        protocol: Protocol::Http,
        direction: Direction::Inbound,
        origin: Origin::Observed,
        operation: "POST /stripe/payment_intents".into(),
        started_at: Utc::now(),
        duration_ms: 2,
        request: json!({"method":"POST","path":"/stripe/payment_intents","body":"{\"amount\":4999}"}),
        response: json!({"status":200}),
        source_interaction_id: None,
        context: Default::default(),
    }
}

#[test]
fn contract_passes_when_expected_subset_matches() {
    let interaction = interaction();
    let contract = Contract {
        id: Uuid::now_v7(),
        name: "stripe payment intent".into(),
        operation: Some(json!("POST /stripe/payment_intents")),
        request: Some(json!({"method":"POST","path":"/stripe/payment_intents"})),
        response: Some(json!({"status":200})),
    };

    let result = assert_interaction(&contract, &interaction);

    assert!(result.passed);
    assert!(result.mismatches.is_empty());
}

#[test]
fn contract_returns_machine_readable_mismatch_evidence() {
    let interaction = interaction();
    let contract = Contract {
        id: Uuid::now_v7(),
        name: "stripe failure".into(),
        operation: Some(json!("POST /stripe/payment_intents")),
        request: None,
        response: Some(json!({"status":201})),
    };

    let result = assert_interaction(&contract, &interaction);

    assert!(!result.passed);
    assert_eq!(result.mismatches.len(), 1);
    assert_eq!(result.mismatches[0].path, "response");
    assert_eq!(result.mismatches[0].expected, json!({"status":201}));
    assert_eq!(result.mismatches[0].actual, json!({"status":200}));
}
