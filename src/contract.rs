//! Deterministic contract assertions.
//!
//! JSON objects use subset matching: a contract specifies only the fields that
//! matter, while captured interactions may contain additional evidence.

use serde_json::Value;
use uuid::Uuid;

use crate::domain::{AssertionResult, Contract, Interaction, Mismatch};

pub fn assert_interaction(contract: &Contract, interaction: &Interaction) -> AssertionResult {
    let mut mismatches = Vec::new();

    check(
        &mut mismatches,
        "operation",
        contract.operation.as_ref(),
        Some(&Value::String(interaction.operation.clone())),
    );
    check(
        &mut mismatches,
        "request",
        contract.request.as_ref(),
        Some(&interaction.request),
    );
    check(
        &mut mismatches,
        "response",
        contract.response.as_ref(),
        Some(&interaction.response),
    );

    AssertionResult {
        id: Uuid::now_v7(),
        contract_id: contract.id,
        interaction_id: interaction.id,
        passed: mismatches.is_empty(),
        mismatches,
    }
}

fn check(
    mismatches: &mut Vec<Mismatch>,
    path: &str,
    expected: Option<&Value>,
    actual: Option<&Value>,
) {
    let Some(expected) = expected else { return };
    let actual = actual.cloned().unwrap_or(Value::Null);
    if !contains(&actual, expected) {
        mismatches.push(Mismatch {
            path: path.to_owned(),
            expected: expected.clone(),
            actual,
        });
    }
}

fn contains(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(actual), Value::Object(expected)) => {
            expected.iter().all(|(key, expected)| {
                actual
                    .get(key)
                    .is_some_and(|actual| contains(actual, expected))
            })
        }
        _ => actual == expected,
    }
}
