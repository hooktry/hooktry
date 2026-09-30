use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    Http,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Observed,
    Proxied,
    Emulated,
    Replayed,
    Generated,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CorrelationContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub causation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

impl CorrelationContext {
    pub fn is_empty(&self) -> bool {
        self.trace_id.is_none()
            && self.parent_span_id.is_none()
            && self.request_id.is_none()
            && self.correlation_id.is_none()
            && self.causation_id.is_none()
            && self.message_id.is_none()
            && self.idempotency_key.is_none()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct InteractionContext {
    #[serde(default, skip_serializing_if = "CorrelationContext::is_empty")]
    pub correlation: CorrelationContext,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attributes: BTreeMap<String, serde_json::Value>,
}

impl InteractionContext {
    pub fn is_empty(&self) -> bool {
        self.correlation.is_empty() && self.attributes.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Interaction {
    pub id: Uuid,
    pub session_id: Uuid,
    pub protocol: Protocol,
    pub direction: Direction,
    pub origin: Origin,
    pub operation: String,
    pub started_at: DateTime<Utc>,
    pub duration_ms: u64,
    pub request: serde_json::Value,
    pub response: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_interaction_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "InteractionContext::is_empty")]
    pub context: InteractionContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub interaction_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExposureMode {
    Forward,
    Relay,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExposureAccess {
    Private,
    Workspace,
    Public,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExposureState {
    Active,
    Revoked,
    Expired,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExposureTarget {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exposure {
    pub id: Uuid,
    pub session_id: Uuid,
    pub name: String,
    pub protocol: Protocol,
    pub mode: ExposureMode,
    pub access: ExposureAccess,
    pub target: ExposureTarget,
    pub url: String,
    pub state: ExposureState,
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contract {
    pub id: Uuid,
    pub name: String,
    pub operation: Option<serde_json::Value>,
    pub request: Option<serde_json::Value>,
    pub response: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mismatch {
    pub path: String,
    pub expected: serde_json::Value,
    pub actual: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub id: Uuid,
    pub contract_id: Uuid,
    pub interaction_id: Uuid,
    pub passed: bool,
    pub mismatches: Vec<Mismatch>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct InteractionCardinality {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScenarioExpectation {
    pub contract_id: Uuid,
    #[serde(default, skip_serializing_if = "InteractionCardinality::is_default")]
    pub cardinality: InteractionCardinality,
}

impl InteractionCardinality {
    pub fn is_default(&self) -> bool {
        self.count.is_none() && self.min.is_none() && self.max.is_none()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub id: Uuid,
    pub name: String,
    pub port: u16,
    pub contract_ids: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub expectations: Vec<ScenarioExpectation>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioRunState {
    AwaitingEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioRun {
    pub id: Uuid,
    pub scenario_id: Uuid,
    pub exposure_id: Uuid,
    pub exposure_url: String,
    pub state: ScenarioRunState,
    pub started_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioCheckOutcome {
    pub contract_id: Uuid,
    #[serde(default)]
    pub cardinality: InteractionCardinality,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub candidate_interaction_ids: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matched_interaction_ids: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub assertion_ids: Vec<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interaction_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assertion_id: Option<Uuid>,
    pub passed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioOutcome {
    pub run_id: Uuid,
    pub scenario_id: Uuid,
    pub completed_at: DateTime<Utc>,
    pub passed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recording_id: Option<Uuid>,
    pub replayed_interaction_ids: Vec<Uuid>,
    pub checks: Vec<ScenarioCheckOutcome>,
}
