use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::{Instant, sleep_until};
use uuid::Uuid;

use crate::{
    contract::assert_interaction,
    domain::{
        Contract, CorrelationContext, Interaction, InteractionCardinality, Origin, Scenario,
        ScenarioCheckOutcome,
        ScenarioExpectation, ScenarioObservation, ScenarioOutcome, ScenarioRun, ScenarioRunState,
    },
    exposure::{ExposureError, ExposureService},
    recording::{ReplayError, replay, snapshot},
    store::InteractionStore,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioContractSpec {
    pub name: String,
    pub operation: String,
    pub request: Option<Value>,
    pub response: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<CorrelationContext>,
    #[serde(default)]
    pub count: Option<usize>,
    #[serde(default)]
    pub min: Option<usize>,
    #[serde(default)]
    pub max: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioManifest {
    pub name: String,
    pub target: ScenarioTarget,
    #[serde(default)]
    pub observation: ScenarioObservation,
    pub contracts: Vec<ScenarioContractSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioTarget {
    pub port: u16,
}

impl From<ScenarioManifest> for CreateScenario {
    fn from(manifest: ScenarioManifest) -> Self {
        Self {
            name: manifest.name,
            port: manifest.target.port,
            observation: manifest.observation,
            contracts: manifest.contracts,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateScenario {
    pub name: String,
    pub port: u16,
    #[serde(default)]
    pub observation: ScenarioObservation,
    pub contracts: Vec<ScenarioContractSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioError {
    InvalidName,
    InvalidPort,
    ContractRequired,
    InvalidContractName,
    InvalidOperation,
    InvalidCardinality,
    InvalidObservation,
    ScenarioNotFound,
    RunNotFound,
    ContractNotFound(Uuid),
    Exposure(ExposureError),
    Replay(ReplayError),
}

struct ObservationState {
    satisfied: bool,
    exceeded: bool,
    requires_full_window: bool,
}

pub fn outcome_exit_code(outcome: &ScenarioOutcome) -> i32 {
    if outcome.passed { 0 } else { 1 }
}

pub fn create(
    store: &InteractionStore,
    request: CreateScenario,
) -> Result<Scenario, ScenarioError> {
    if request.name.trim().is_empty() {
        return Err(ScenarioError::InvalidName);
    }
    if request.port == 0 {
        return Err(ScenarioError::InvalidPort);
    }
    if request.contracts.is_empty() {
        return Err(ScenarioError::ContractRequired);
    }
    validate_observation(&request.observation)?;

    for spec in &request.contracts {
        if spec.name.trim().is_empty() {
            return Err(ScenarioError::InvalidContractName);
        }
        if spec.operation.trim().is_empty() {
            return Err(ScenarioError::InvalidOperation);
        }
        validate_cardinality(&cardinality(spec))?;
    }

    let mut contract_ids = Vec::with_capacity(request.contracts.len());
    let mut expectations = Vec::with_capacity(request.contracts.len());
    for spec in request.contracts {
        let contract = Contract {
            id: Uuid::now_v7(),
            name: spec.name,
            operation: Some(json!(spec.operation)),
            request: spec.request,
            response: spec.response,
            context: spec.context,
        };
        store.save_contract(&contract);
        contract_ids.push(contract.id);
        expectations.push(ScenarioExpectation {
            contract_id: contract.id,
            cardinality: InteractionCardinality {
                count: spec.count,
                min: spec.min,
                max: spec.max,
            },
        });
    }

    let scenario = Scenario {
        id: Uuid::now_v7(),
        name: request.name,
        port: request.port,
        contract_ids,
        expectations,
        observation: request.observation,
        created_at: Utc::now(),
    };
    store.save_scenario(&scenario);
    Ok(scenario)
}

pub fn start(
    store: &InteractionStore,
    exposures: &ExposureService,
    session_id: Uuid,
    scenario_id: Uuid,
) -> Result<ScenarioRun, ScenarioError> {
    let scenario = store
        .scenario(scenario_id)
        .ok_or(ScenarioError::ScenarioNotFound)?;
    let run_id = Uuid::now_v7();
    let exposure = exposures
        .create(
            session_id,
            format!("scenario:{}:{run_id}", scenario.id),
            scenario.port,
        )
        .map_err(ScenarioError::Exposure)?;
    let run = ScenarioRun {
        id: run_id,
        scenario_id: scenario.id,
        exposure_id: exposure.id,
        exposure_url: exposure.url,
        state: ScenarioRunState::AwaitingEvidence,
        started_at: Utc::now(),
    };
    store.save_scenario_run(&run);
    Ok(run)
}

pub async fn complete(
    store: &InteractionStore,
    exposures: &ExposureService,
    session_id: Uuid,
    run_id: Uuid,
) -> Result<ScenarioOutcome, ScenarioError> {
    if let Some(outcome) = store.scenario_outcome(run_id) {
        return Ok(outcome);
    }

    let run = store
        .scenario_run(run_id)
        .ok_or(ScenarioError::RunNotFound)?;
    let scenario = store
        .scenario(run.scenario_id)
        .ok_or(ScenarioError::ScenarioNotFound)?;
    let definitions = expectation_contracts(store, &scenario)?;
    let observation_started = Instant::now();
    let sources =
        observe_sources(store, run.exposure_id, &scenario.observation, &definitions).await;
    let observation_elapsed_ms = observation_started.elapsed().as_millis() as u64;

    let (recording_id, replayed) = if sources.is_empty() {
        (None, Vec::new())
    } else {
        let recording = snapshot(store, sources.iter().map(|item| item.id).collect());
        let replayed = replay(store, session_id, recording.id).map_err(ScenarioError::Replay)?;
        (Some(recording.id), replayed)
    };

    let mut checks = Vec::with_capacity(definitions.len());
    for (expectation, contract) in definitions {
        let operation = contract
            .operation
            .as_ref()
            .and_then(Value::as_str)
            .ok_or(ScenarioError::InvalidOperation)?;

        let candidates = replayed
            .iter()
            .filter(|item| item.operation == operation)
            .collect::<Vec<_>>();
        let mut assertion_ids = Vec::with_capacity(candidates.len());
        let mut matched_interaction_ids = Vec::new();
        let mut matched_assertion_ids = Vec::new();

        for interaction in &candidates {
            let assertion = assert_interaction(&contract, interaction);
            store.save_assertion(&assertion);
            assertion_ids.push(assertion.id);
            if assertion.passed {
                matched_interaction_ids.push(interaction.id);
                matched_assertion_ids.push(assertion.id);
            }
        }

        let passed = cardinality_matches(&expectation.cardinality, matched_interaction_ids.len());
        let error = (!passed).then(|| {
            format!(
                "expected {}, observed {} matching interactions",
                cardinality_description(&expectation.cardinality),
                matched_interaction_ids.len()
            )
        });
        let interaction_id =
            (matched_interaction_ids.len() == 1).then(|| matched_interaction_ids[0]);
        let assertion_id = (matched_assertion_ids.len() == 1).then(|| matched_assertion_ids[0]);

        checks.push(ScenarioCheckOutcome {
            contract_id: contract.id,
            cardinality: expectation.cardinality,
            candidate_interaction_ids: candidates.iter().map(|item| item.id).collect(),
            matched_interaction_ids,
            assertion_ids,
            interaction_id,
            assertion_id,
            passed,
            error,
        });
    }

    let passed = !checks.is_empty() && checks.iter().all(|check| check.passed);
    let outcome = ScenarioOutcome {
        run_id: run.id,
        scenario_id: scenario.id,
        completed_at: Utc::now(),
        passed,
        observation: scenario.observation,
        observation_elapsed_ms,
        recording_id,
        replayed_interaction_ids: replayed.iter().map(|item| item.id).collect(),
        checks,
    };
    store.save_scenario_outcome(&outcome);

    let _ = exposures.revoke(run.exposure_id);

    Ok(outcome)
}

async fn observe_sources(
    store: &InteractionStore,
    exposure_id: Uuid,
    observation: &ScenarioObservation,
    definitions: &[(ScenarioExpectation, Contract)],
) -> Vec<Interaction> {
    let mut revision = store.subscribe_interactions();
    let mut sources = source_interactions(store, exposure_id);
    if observation.within_ms == 0 {
        return sources;
    }

    let started = Instant::now();
    let deadline = started + Duration::from_millis(observation.within_ms);
    let settle = Duration::from_millis(observation.settle_ms);
    let mut last_relevant_activity = started;
    let mut seen_relevant = sources.len();

    loop {
        let state = observation_state(definitions, &sources);
        if state.exceeded {
            return sources;
        }

        let now = Instant::now();
        if now >= deadline {
            return source_interactions(store, exposure_id);
        }

        let can_settle_early =
            observation.settle_ms > 0 && state.satisfied && !state.requires_full_window;
        let wake_at = if can_settle_early {
            let settle_deadline = last_relevant_activity + settle;
            if now >= settle_deadline {
                return sources;
            }
            settle_deadline.min(deadline)
        } else {
            deadline
        };

        tokio::select! {
            changed = revision.changed() => {
                if changed.is_err() {
                    return sources;
                }

                let updated = source_interactions(store, exposure_id);
                if updated.len() != seen_relevant {
                    seen_relevant = updated.len();
                    last_relevant_activity = Instant::now();
                }
                sources = updated;
            }
            _ = sleep_until(wake_at) => {
                let updated = source_interactions(store, exposure_id);
                if updated.len() != seen_relevant {
                    seen_relevant = updated.len();
                    last_relevant_activity = Instant::now();
                    sources = updated;
                    continue;
                }

                if Instant::now() >= deadline {
                    return updated;
                }
                return updated;
            }
        }
    }
}

fn source_interactions(store: &InteractionStore, exposure_id: Uuid) -> Vec<Interaction> {
    let exposure_id = exposure_id.to_string();
    store
        .all()
        .into_iter()
        .filter(|interaction| {
            interaction.origin == Origin::Proxied
                && interaction.request["exposure_id"].as_str() == Some(exposure_id.as_str())
        })
        .collect()
}

fn expectation_contracts(
    store: &InteractionStore,
    scenario: &Scenario,
) -> Result<Vec<(ScenarioExpectation, Contract)>, ScenarioError> {
    scenario_expectations(scenario)
        .into_iter()
        .map(|expectation| {
            let contract = store
                .contract(expectation.contract_id)
                .ok_or(ScenarioError::ContractNotFound(expectation.contract_id))?;
            Ok((expectation, contract))
        })
        .collect()
}

fn observation_state(
    definitions: &[(ScenarioExpectation, Contract)],
    interactions: &[Interaction],
) -> ObservationState {
    let mut satisfied = true;
    let mut exceeded = false;
    let mut requires_full_window = false;

    for (expectation, contract) in definitions {
        let operation = contract.operation.as_ref().and_then(Value::as_str);
        let matched = interactions
            .iter()
            .filter(|interaction| Some(interaction.operation.as_str()) == operation)
            .filter(|interaction| assert_interaction(contract, interaction).passed)
            .count();

        satisfied &= cardinality_matches(&expectation.cardinality, matched);
        exceeded |= maximum_allowed(&expectation.cardinality).is_some_and(|max| matched > max);
        requires_full_window |= minimum_required(&expectation.cardinality) == 0;
    }

    ObservationState {
        satisfied,
        exceeded,
        requires_full_window,
    }
}

fn cardinality(spec: &ScenarioContractSpec) -> InteractionCardinality {
    InteractionCardinality {
        count: spec.count,
        min: spec.min,
        max: spec.max,
    }
}

fn validate_cardinality(cardinality: &InteractionCardinality) -> Result<(), ScenarioError> {
    if cardinality.count.is_some() && (cardinality.min.is_some() || cardinality.max.is_some()) {
        return Err(ScenarioError::InvalidCardinality);
    }
    if let (Some(min), Some(max)) = (cardinality.min, cardinality.max)
        && min > max
    {
        return Err(ScenarioError::InvalidCardinality);
    }
    Ok(())
}

fn validate_observation(observation: &ScenarioObservation) -> Result<(), ScenarioError> {
    if observation.within_ms == 0 && observation.settle_ms > 0 {
        return Err(ScenarioError::InvalidObservation);
    }
    if observation.within_ms > 0 && observation.settle_ms > observation.within_ms {
        return Err(ScenarioError::InvalidObservation);
    }
    Ok(())
}

fn scenario_expectations(scenario: &Scenario) -> Vec<ScenarioExpectation> {
    if scenario.expectations.is_empty() {
        scenario
            .contract_ids
            .iter()
            .map(|contract_id| ScenarioExpectation {
                contract_id: *contract_id,
                cardinality: InteractionCardinality::default(),
            })
            .collect()
    } else {
        scenario.expectations.clone()
    }
}

fn cardinality_matches(cardinality: &InteractionCardinality, observed: usize) -> bool {
    if let Some(count) = cardinality.count {
        return observed == count;
    }
    if cardinality.is_default() {
        return observed == 1;
    }

    let min = cardinality.min.unwrap_or(0);
    let max = cardinality.max.unwrap_or(usize::MAX);
    observed >= min && observed <= max
}

fn minimum_required(cardinality: &InteractionCardinality) -> usize {
    if let Some(count) = cardinality.count {
        return count;
    }
    if cardinality.is_default() {
        return 1;
    }
    cardinality.min.unwrap_or(0)
}

fn maximum_allowed(cardinality: &InteractionCardinality) -> Option<usize> {
    if let Some(count) = cardinality.count {
        return Some(count);
    }
    if cardinality.is_default() {
        return Some(1);
    }
    cardinality.max
}

fn cardinality_description(cardinality: &InteractionCardinality) -> String {
    if let Some(count) = cardinality.count {
        return format!("exactly {count}");
    }
    match (cardinality.min, cardinality.max) {
        (Some(min), Some(max)) => format!("between {min} and {max}"),
        (Some(min), None) => format!("at least {min}"),
        (None, Some(max)) => format!("at most {max}"),
        (None, None) => "exactly 1".to_owned(),
    }
}
