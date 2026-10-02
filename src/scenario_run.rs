use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{ScenarioOutcome, ScenarioRun};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioCommandResult {
    pub command: Vec<String>,
    pub exit_code: Option<i32>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioRunReport {
    pub scenario_id: Uuid,
    pub run_id: Uuid,
    pub exposure_id: Uuid,
    pub exposure_url: String,
    pub command: ScenarioCommandResult,
    pub outcome: ScenarioOutcome,
    pub passed: bool,
}

pub fn environment(base_url: &str, run: &ScenarioRun) -> Vec<(String, String)> {
    vec![
        ("HOOKTRY_BASE_URL".to_owned(), base_url.to_owned()),
        ("HOOKTRY_SCENARIO_ID".to_owned(), run.scenario_id.to_string()),
        ("HOOKTRY_SCENARIO_RUN_ID".to_owned(), run.id.to_string()),
        ("HOOKTRY_EXPOSURE_ID".to_owned(), run.exposure_id.to_string()),
        ("HOOKTRY_EXPOSURE_URL".to_owned(), run.exposure_url.clone()),
    ]
}

pub fn report(
    run: &ScenarioRun,
    command: Vec<String>,
    exit_code: Option<i32>,
    outcome: ScenarioOutcome,
) -> ScenarioRunReport {
    let command_success = exit_code == Some(0);
    let passed = command_success && outcome.passed;

    ScenarioRunReport {
        scenario_id: run.scenario_id,
        run_id: run.id,
        exposure_id: run.exposure_id,
        exposure_url: run.exposure_url.clone(),
        command: ScenarioCommandResult {
            command,
            exit_code,
            success: command_success,
        },
        outcome,
        passed,
    }
}

pub fn exit_code(report: &ScenarioRunReport) -> i32 {
    if report.passed { 0 } else { 1 }
}
