use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    contract::assert_interaction,
    domain::{
        Contract, Origin, Scenario, ScenarioCheckOutcome, ScenarioOutcome, ScenarioRun,
        ScenarioRunState,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateScenario {
    pub name: String,
    pub port: u16,
    pub contracts: Vec<ScenarioContractSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioError {
    InvalidName,
    InvalidPort,
    ContractRequired,
    InvalidContractName,
    InvalidOperation,
    ScenarioNotFound,
    RunNotFound,
    ContractNotFound(Uuid),
    Exposure(ExposureError),
    Replay(ReplayError),
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

    let mut contract_ids = Vec::with_capacity(request.contracts.len());
    for spec in request.contracts {
        if spec.name.trim().is_empty() {
            return Err(ScenarioError::InvalidContractName);
        }
        if spec.operation.trim().is_empty() {
            return Err(ScenarioError::InvalidOperation);
        }

        let contract = Contract {
            id: Uuid::now_v7(),
            name: spec.name,
            operation: Some(json!(spec.operation)),
            request: spec.request,
            response: spec.response,
        };
        store.save_contract(&contract);
        contract_ids.push(contract.id);
    }

    let scenario = Scenario {
        id: Uuid::now_v7(),
        name: request.name,
        port: request.port,
        contract_ids,
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

pub fn complete(
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
    let exposure_id = run.exposure_id.to_string();

    let sources = store
        .all()
        .into_iter()
        .filter(|interaction| {
            interaction.origin == Origin::Proxied
                && interaction.request["exposure_id"].as_str() == Some(exposure_id.as_str())
        })
        .collect::<Vec<_>>();

    let (recording_id, replayed) = if sources.is_empty() {
        (None, Vec::new())
    } else {
        let recording = snapshot(store, sources.iter().map(|item| item.id).collect());
        let replayed =
            replay(store, session_id, recording.id).map_err(ScenarioError::Replay)?;
        (Some(recording.id), replayed)
    };

    let mut checks = Vec::with_capacity(scenario.contract_ids.len());
    for contract_id in &scenario.contract_ids {
        let contract = store
            .contract(*contract_id)
            .ok_or(ScenarioError::ContractNotFound(*contract_id))?;
        let operation = contract
            .operation
            .as_ref()
            .and_then(Value::as_str)
            .ok_or(ScenarioError::InvalidOperation)?;

        if let Some(interaction) = replayed.iter().find(|item| item.operation == operation) {
            let assertion = assert_interaction(&contract, interaction);
            store.save_assertion(&assertion);
            checks.push(ScenarioCheckOutcome {
                contract_id: contract.id,
                interaction_id: Some(interaction.id),
                assertion_id: Some(assertion.id),
                passed: assertion.passed,
                error: None,
            });
        } else {
            checks.push(ScenarioCheckOutcome {
                contract_id: contract.id,
                interaction_id: None,
                assertion_id: None,
                passed: false,
                error: Some(format!("no replayed interaction matched operation {operation}")),
            });
        }
    }

    let passed = !checks.is_empty() && checks.iter().all(|check| check.passed);
    let outcome = ScenarioOutcome {
        run_id: run.id,
        scenario_id: scenario.id,
        completed_at: Utc::now(),
        passed,
        recording_id,
        replayed_interaction_ids: replayed.iter().map(|item| item.id).collect(),
        checks,
    };
    store.save_scenario_outcome(&outcome);

    let _ = exposures.revoke(run.exposure_id);

    Ok(outcome)
}
