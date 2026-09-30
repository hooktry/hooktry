use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use postgres::{Client, NoTls};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::execution::{ExecutionOutcome, ExecutionProviderKind, ExecutionRecord};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DurableExecutionState {
    Started,
    Completed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DurableExecutionRecord {
    pub execution_id: Uuid,
    pub workspace_id: Uuid,
    pub provider: ExecutionProviderKind,
    pub state: DurableExecutionState,
    pub started_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<ExecutionOutcome>,
}

impl DurableExecutionRecord {
    pub fn terminal_record(&self) -> Option<ExecutionRecord> {
        match (self.completed_at_unix_ms, self.outcome.clone()) {
            (Some(completed_at_unix_ms), Some(outcome))
                if self.state == DurableExecutionState::Completed =>
            {
                Some(ExecutionRecord {
                    execution_id: self.execution_id,
                    workspace_id: self.workspace_id,
                    provider: self.provider,
                    started_at_unix_ms: self.started_at_unix_ms,
                    completed_at_unix_ms,
                    outcome,
                })
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionStoreError {
    NotFound,
    AlreadyCompleted,
    InvalidRecord(String),
    Storage(String),
}

#[derive(Clone)]
enum ExecutionBackend {
    Sqlite(Arc<Mutex<Connection>>),
    Postgres(Arc<Mutex<Client>>),
}

#[derive(Clone)]
pub struct ExecutionStore {
    backend: ExecutionBackend,
}

impl Default for ExecutionStore {
    fn default() -> Self {
        Self::in_memory().expect("create in-memory execution store")
    }
}

impl ExecutionStore {
    pub fn in_memory() -> Result<Self, ExecutionStoreError> {
        let connection = Connection::open_in_memory()
            .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?;
        Self::from_sqlite_connection(connection)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, ExecutionStoreError> {
        let connection =
            Connection::open(path).map_err(|error| ExecutionStoreError::Storage(error.to_string()))?;
        Self::from_sqlite_connection(connection)
    }

    pub fn open_postgres(database_url: &str) -> Result<Self, ExecutionStoreError> {
        let mut client = Client::connect(database_url, NoTls)
            .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS hosted_executions (
                    execution_id TEXT PRIMARY KEY,
                    workspace_id TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    started_at BIGINT NOT NULL,
                    completed_at BIGINT,
                    outcome_json TEXT
                );
                CREATE INDEX IF NOT EXISTS hosted_executions_workspace_started
                    ON hosted_executions(workspace_id, started_at DESC);",
            )
            .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?;
        Ok(Self {
            backend: ExecutionBackend::Postgres(Arc::new(Mutex::new(client))),
        })
    }

    fn from_sqlite_connection(connection: Connection) -> Result<Self, ExecutionStoreError> {
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS hosted_executions (
                    execution_id TEXT PRIMARY KEY,
                    workspace_id TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    started_at INTEGER NOT NULL,
                    completed_at INTEGER,
                    outcome_json TEXT
                );
                CREATE INDEX IF NOT EXISTS hosted_executions_workspace_started
                    ON hosted_executions(workspace_id, started_at DESC);",
            )
            .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?;
        Ok(Self {
            backend: ExecutionBackend::Sqlite(Arc::new(Mutex::new(connection))),
        })
    }

    pub async fn reserve_async(
        &self,
        workspace_id: Uuid,
        execution_id: Uuid,
        provider: ExecutionProviderKind,
    ) -> Result<DurableExecutionRecord, ExecutionStoreError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.reserve(workspace_id, execution_id, provider))
            .await
            .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?
    }

    pub async fn complete_async(
        &self,
        record: ExecutionRecord,
    ) -> Result<DurableExecutionRecord, ExecutionStoreError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.complete(&record))
            .await
            .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?
    }

    pub async fn get_async(
        &self,
        workspace_id: Uuid,
        execution_id: Uuid,
    ) -> Result<Option<DurableExecutionRecord>, ExecutionStoreError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.get(workspace_id, execution_id))
            .await
            .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?
    }

    pub async fn discard_started_async(
        &self,
        workspace_id: Uuid,
        execution_id: Uuid,
    ) -> Result<(), ExecutionStoreError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.discard_started(workspace_id, execution_id))
            .await
            .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?
    }

    pub fn reserve(
        &self,
        workspace_id: Uuid,
        execution_id: Uuid,
        provider: ExecutionProviderKind,
    ) -> Result<DurableExecutionRecord, ExecutionStoreError> {
        let started_at_unix_ms = unix_time_ms();
        let started_at = millis_i64(started_at_unix_ms)?;
        let provider = provider_name(provider);

        match &self.backend {
            ExecutionBackend::Sqlite(connection) => {
                connection
                    .lock()
                    .expect("execution store poisoned")
                    .execute(
                        "INSERT INTO hosted_executions
                            (execution_id, workspace_id, provider, started_at, completed_at, outcome_json)
                         VALUES (?1, ?2, ?3, ?4, NULL, NULL)",
                        params![
                            execution_id.to_string(),
                            workspace_id.to_string(),
                            provider,
                            started_at
                        ],
                    )
                    .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?;
            }
            ExecutionBackend::Postgres(client) => {
                client
                    .lock()
                    .expect("execution store poisoned")
                    .execute(
                        "INSERT INTO hosted_executions
                            (execution_id, workspace_id, provider, started_at, completed_at, outcome_json)
                         VALUES ($1, $2, $3, $4, NULL, NULL)",
                        &[
                            &execution_id.to_string(),
                            &workspace_id.to_string(),
                            &provider,
                            &started_at,
                        ],
                    )
                    .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?;
            }
        }

        Ok(DurableExecutionRecord {
            execution_id,
            workspace_id,
            provider: parse_provider(provider)?,
            state: DurableExecutionState::Started,
            started_at_unix_ms,
            completed_at_unix_ms: None,
            outcome: None,
        })
    }

    pub fn complete(
        &self,
        record: &ExecutionRecord,
    ) -> Result<DurableExecutionRecord, ExecutionStoreError> {
        let completed_at = millis_i64(record.completed_at_unix_ms)?;
        let started_at = millis_i64(record.started_at_unix_ms)?;
        let outcome_json = serde_json::to_string(&record.outcome)
            .map_err(|error| ExecutionStoreError::InvalidRecord(error.to_string()))?;
        let provider = provider_name(record.provider);
        let execution_id = record.execution_id.to_string();
        let workspace_id = record.workspace_id.to_string();

        let updated = match &self.backend {
            ExecutionBackend::Sqlite(connection) => connection
                .lock()
                .expect("execution store poisoned")
                .execute(
                    "UPDATE hosted_executions
                     SET completed_at=?1, outcome_json=?2
                     WHERE execution_id=?3 AND workspace_id=?4 AND provider=?5
                       AND started_at=?6 AND outcome_json IS NULL",
                    params![
                        completed_at,
                        outcome_json,
                        execution_id,
                        workspace_id,
                        provider,
                        started_at
                    ],
                )
                .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?,
            ExecutionBackend::Postgres(client) => client
                .lock()
                .expect("execution store poisoned")
                .execute(
                    "UPDATE hosted_executions
                     SET completed_at=$1, outcome_json=$2
                     WHERE execution_id=$3 AND workspace_id=$4 AND provider=$5
                       AND started_at=$6 AND outcome_json IS NULL",
                    &[
                        &completed_at,
                        &outcome_json,
                        &execution_id,
                        &workspace_id,
                        &provider,
                        &started_at,
                    ],
                )
                .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?
                as usize,
        };

        if updated == 0 {
            return match self.get(record.workspace_id, record.execution_id)? {
                None => Err(ExecutionStoreError::NotFound),
                Some(existing) if existing.state == DurableExecutionState::Completed => {
                    Err(ExecutionStoreError::AlreadyCompleted)
                }
                Some(_) => Err(ExecutionStoreError::InvalidRecord(
                    "reserved execution envelope does not match terminal record".to_owned(),
                )),
            };
        }

        Ok(DurableExecutionRecord {
            execution_id: record.execution_id,
            workspace_id: record.workspace_id,
            provider: record.provider,
            state: DurableExecutionState::Completed,
            started_at_unix_ms: record.started_at_unix_ms,
            completed_at_unix_ms: Some(record.completed_at_unix_ms),
            outcome: Some(record.outcome.clone()),
        })
    }

    pub fn get(
        &self,
        workspace_id: Uuid,
        execution_id: Uuid,
    ) -> Result<Option<DurableExecutionRecord>, ExecutionStoreError> {
        let row = match &self.backend {
            ExecutionBackend::Sqlite(connection) => connection
                .lock()
                .expect("execution store poisoned")
                .query_row(
                    "SELECT workspace_id, provider, started_at, completed_at, outcome_json
                     FROM hosted_executions WHERE execution_id=?1",
                    [execution_id.to_string()],
                    |row| {
                        Ok(StoredExecutionRow {
                            workspace_id: row.get(0)?,
                            provider: row.get(1)?,
                            started_at: row.get(2)?,
                            completed_at: row.get(3)?,
                            outcome_json: row.get(4)?,
                        })
                    },
                )
                .optional()
                .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?,
            ExecutionBackend::Postgres(client) => client
                .lock()
                .expect("execution store poisoned")
                .query_opt(
                    "SELECT workspace_id, provider, started_at, completed_at, outcome_json
                     FROM hosted_executions WHERE execution_id=$1",
                    &[&execution_id.to_string()],
                )
                .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?
                .map(|row| StoredExecutionRow {
                    workspace_id: row.get(0),
                    provider: row.get(1),
                    started_at: row.get(2),
                    completed_at: row.get(3),
                    outcome_json: row.get(4),
                }),
        };

        let Some(row) = row else {
            return Ok(None);
        };
        let record = row.into_record(execution_id)?;
        if record.workspace_id != workspace_id {
            return Ok(None);
        }
        Ok(Some(record))
    }

    pub fn discard_started(
        &self,
        workspace_id: Uuid,
        execution_id: Uuid,
    ) -> Result<(), ExecutionStoreError> {
        match &self.backend {
            ExecutionBackend::Sqlite(connection) => {
                connection
                    .lock()
                    .expect("execution store poisoned")
                    .execute(
                        "DELETE FROM hosted_executions
                         WHERE execution_id=?1 AND workspace_id=?2 AND outcome_json IS NULL",
                        params![execution_id.to_string(), workspace_id.to_string()],
                    )
                    .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?;
            }
            ExecutionBackend::Postgres(client) => {
                client
                    .lock()
                    .expect("execution store poisoned")
                    .execute(
                        "DELETE FROM hosted_executions
                         WHERE execution_id=$1 AND workspace_id=$2 AND outcome_json IS NULL",
                        &[&execution_id.to_string(), &workspace_id.to_string()],
                    )
                    .map_err(|error| ExecutionStoreError::Storage(error.to_string()))?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct StoredExecutionRow {
    workspace_id: String,
    provider: String,
    started_at: i64,
    completed_at: Option<i64>,
    outcome_json: Option<String>,
}

impl StoredExecutionRow {
    fn into_record(
        self,
        execution_id: Uuid,
    ) -> Result<DurableExecutionRecord, ExecutionStoreError> {
        let workspace_id = parse_uuid(&self.workspace_id)?;
        let provider = parse_provider(&self.provider)?;
        let started_at_unix_ms = millis_u64(self.started_at)?;
        match (self.completed_at, self.outcome_json) {
            (None, None) => Ok(DurableExecutionRecord {
                execution_id,
                workspace_id,
                provider,
                state: DurableExecutionState::Started,
                started_at_unix_ms,
                completed_at_unix_ms: None,
                outcome: None,
            }),
            (Some(completed_at), Some(outcome_json)) => Ok(DurableExecutionRecord {
                execution_id,
                workspace_id,
                provider,
                state: DurableExecutionState::Completed,
                started_at_unix_ms,
                completed_at_unix_ms: Some(millis_u64(completed_at)?),
                outcome: Some(
                    serde_json::from_str(&outcome_json)
                        .map_err(|error| ExecutionStoreError::InvalidRecord(error.to_string()))?,
                ),
            }),
            _ => Err(ExecutionStoreError::InvalidRecord(
                "completed_at and outcome_json must transition together".to_owned(),
            )),
        }
    }
}

fn provider_name(provider: ExecutionProviderKind) -> &'static str {
    match provider {
        ExecutionProviderKind::Http => "http",
    }
}

fn parse_provider(value: &str) -> Result<ExecutionProviderKind, ExecutionStoreError> {
    match value {
        "http" => Ok(ExecutionProviderKind::Http),
        _ => Err(ExecutionStoreError::InvalidRecord(format!(
            "invalid execution provider: {value}"
        ))),
    }
}

fn parse_uuid(value: &str) -> Result<Uuid, ExecutionStoreError> {
    value
        .parse()
        .map_err(|error| ExecutionStoreError::InvalidRecord(format!("invalid UUID: {error}")))
}

fn millis_i64(value: u64) -> Result<i64, ExecutionStoreError> {
    i64::try_from(value).map_err(|error| ExecutionStoreError::InvalidRecord(error.to_string()))
}

fn millis_u64(value: i64) -> Result<u64, ExecutionStoreError> {
    u64::try_from(value).map_err(|error| ExecutionStoreError::InvalidRecord(error.to_string()))
}

fn unix_time_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}
