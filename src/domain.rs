use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Protocol { Http }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction { Inbound, Outbound }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Origin { Observed, Proxied, Emulated, Replayed, Generated }

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session { pub id: Uuid, pub started_at: DateTime<Utc> }
