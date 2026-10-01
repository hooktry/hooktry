use std::{
    fs::OpenOptions,
    io::Write,
    path::Path,
    time::Duration,
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    domain::ScenarioOrdering,
    scenario::CreateScenario,
    scenario_run::ScenarioRunReport,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioUsageFeatures {
    pub contract_count: usize,
    pub exact_cardinality: bool,
    pub ranged_cardinality: bool,
    pub ordering: bool,
    pub observation_horizon: bool,
    pub settle_window: bool,
    pub context_match: bool,
    pub idempotency_context: bool,
    pub duplicate_guard: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioUsageEvent {
    pub schema_version: u8,
    pub event_id: Uuid,
    pub event: String,
    pub occurred_at_unix_ms: i64,
    pub passed: bool,
    pub command_success: bool,
    pub outcome_passed: bool,
    pub check_count: usize,
    pub observation_elapsed_ms: u64,
    pub features: ScenarioUsageFeatures,
}

impl ScenarioUsageFeatures {
    pub fn from_request(request: &CreateScenario) -> Self {
        let exact_cardinality = request.contracts.iter().any(|contract| {
            contract.count.is_some()
                || (contract.count.is_none() && contract.min.is_none() && contract.max.is_none())
        });
        let ranged_cardinality = request
            .contracts
            .iter()
            .any(|contract| contract.min.is_some() || contract.max.is_some());
        let context_match = request
            .contracts
            .iter()
            .any(|contract| contract.context.as_ref().is_some_and(|context| !context.is_empty()));
        let idempotency_context = request.contracts.iter().any(|contract| {
            contract
                .context
                .as_ref()
                .and_then(|context| context.idempotency_key.as_ref())
                .is_some()
        });
        let duplicate_guard = request.contracts.iter().any(|contract| {
            let has_upper_bound = contract.count.is_some() || contract.max.is_some();
            let has_idempotency = contract
                .context
                .as_ref()
                .and_then(|context| context.idempotency_key.as_ref())
                .is_some();
            has_upper_bound && has_idempotency
        });

        Self {
            contract_count: request.contracts.len(),
            exact_cardinality,
            ranged_cardinality,
            ordering: request.ordering == Some(ScenarioOrdering::Declared),
            observation_horizon: request.observation.within_ms > 0,
            settle_window: request.observation.settle_ms > 0,
            context_match,
            idempotency_context,
            duplicate_guard,
        }
    }
}

impl ScenarioUsageEvent {
    pub fn completed(features: ScenarioUsageFeatures, report: &ScenarioRunReport) -> Self {
        Self {
            schema_version: 1,
            event_id: Uuid::now_v7(),
            event: "scenario_run_completed".to_owned(),
            occurred_at_unix_ms: Utc::now().timestamp_millis(),
            passed: report.passed,
            command_success: report.command.success,
            outcome_passed: report.outcome.passed,
            check_count: report.outcome.checks.len(),
            observation_elapsed_ms: report.outcome.observation_elapsed_ms,
            features,
        }
    }
}

pub async fn emit_from_env(event: &ScenarioUsageEvent) -> Result<(), String> {
    let mut errors = Vec::new();

    if let Ok(path) = std::env::var("ORTYO_USAGE_LOG")
        && !path.trim().is_empty()
        && let Err(error) = append_jsonl(Path::new(&path), event)
    {
        errors.push(error);
    }

    if let Ok(endpoint) = std::env::var("ORTYO_USAGE_ENDPOINT")
        && !endpoint.trim().is_empty()
        && let Err(error) = post_event(&endpoint, event).await
    {
        errors.push(error);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn append_jsonl(path: &Path, event: &ScenarioUsageEvent) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("open usage log {}: {error}", path.display()))?;
    serde_json::to_writer(&mut file, event)
        .map_err(|error| format!("serialize usage event: {error}"))?;
    file.write_all(b"\n")
        .map_err(|error| format!("write usage log {}: {error}", path.display()))
}

async fn post_event(endpoint: &str, event: &ScenarioUsageEvent) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|error| format!("build usage client: {error}"))?;
    let mut request = client.post(endpoint).json(event);

    if let Ok(token) = std::env::var("ORTYO_USAGE_TOKEN")
        && !token.trim().is_empty()
    {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .map_err(|error| format!("send usage event: {error}"))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("usage endpoint returned HTTP {}", response.status()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::{CorrelationContext, ScenarioObservation},
        scenario::ScenarioContractSpec,
    };

    #[test]
    fn classifies_duplicate_and_temporal_features_without_payload_data() {
        let request = CreateScenario {
            name: "sensitive scenario name".to_owned(),
            port: 4321,
            observation: ScenarioObservation {
                within_ms: 5000,
                settle_ms: 500,
            },
            ordering: None,
            contracts: vec![ScenarioContractSpec {
                name: "sensitive contract name".to_owned(),
                operation: "POST /secret".to_owned(),
                request: Some(serde_json::json!({"body":"secret-body"})),
                response: None,
                context: Some(CorrelationContext {
                    idempotency_key: Some("secret-idempotency".to_owned()),
                    ..Default::default()
                }),
                count: Some(1),
                min: None,
                max: None,
            }],
        };

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

        let serialized = serde_json::to_string(&features).unwrap();
        assert!(!serialized.contains("sensitive"));
        assert!(!serialized.contains("/secret"));
        assert!(!serialized.contains("secret-body"));
        assert!(!serialized.contains("secret-idempotency"));
        assert!(!serialized.contains("4321"));
    }

    #[test]
    fn classifies_declared_order_without_event_identity() {
        let request = CreateScenario {
            name: "order".to_owned(),
            port: 3000,
            observation: Default::default(),
            ordering: Some(ScenarioOrdering::Declared),
            contracts: vec![
                ScenarioContractSpec {
                    name: "a".to_owned(),
                    operation: "POST /a".to_owned(),
                    request: None,
                    response: None,
                    context: None,
                    count: Some(1),
                    min: None,
                    max: None,
                },
                ScenarioContractSpec {
                    name: "b".to_owned(),
                    operation: "POST /b".to_owned(),
                    request: None,
                    response: None,
                    context: None,
                    count: Some(1),
                    min: None,
                    max: None,
                },
            ],
        };

        let features = ScenarioUsageFeatures::from_request(&request);
        assert!(features.ordering);
        assert_eq!(features.contract_count, 2);
        assert!(!features.context_match);
        assert!(!features.duplicate_guard);
    }
}
