use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use postgres::{Client, NoTls};
use reqwest::Url;
use ring::digest::{SHA256, digest};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::execution::HttpExecutionRequest;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    Pending,
    Approved,
    Denied,
    Consumed,
}

impl ApprovalState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Denied => "denied",
            Self::Consumed => "consumed",
        }
    }

    fn parse(value: &str) -> Result<Self, ApprovalError> {
        match value {
            "pending" => Ok(Self::Pending),
            "approved" => Ok(Self::Approved),
            "denied" => Ok(Self::Denied),
            "consumed" => Ok(Self::Consumed),
            _ => Err(ApprovalError::Storage(format!(
                "invalid approval state: {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalDecision {
    Approve,
    Deny,
}

impl ApprovalDecision {
    fn state(self) -> ApprovalState {
        match self {
            Self::Approve => ApprovalState::Approved,
            Self::Deny => ApprovalState::Denied,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalSummary {
    pub method: String,
    pub origin: String,
    pub path: String,
    pub header_names: Vec<String>,
    pub secret_header_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_sha256: Option<String>,
    pub capture_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovalRecord {
    pub approval_id: Uuid,
    pub workspace_id: Uuid,
    pub requested_by_credential_id: Uuid,
    pub request_digest: String,
    pub summary: ApprovalSummary,
    pub state: ApprovalState,
    pub requested_at_unix_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_by_credential_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumed_at_unix_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumed_by_credential_id: Option<Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalError {
    InvalidRequest,
    NotFound,
    NotPending,
    Pending,
    Denied,
    Consumed,
    RequestMismatch,
    Storage(String),
}

#[derive(Clone)]
enum ApprovalBackend {
    Sqlite(Arc<Mutex<Connection>>),
    Postgres(Arc<Mutex<Client>>),
}

#[derive(Clone)]
pub struct ApprovalStore {
    backend: ApprovalBackend,
}

impl Default for ApprovalStore {
    fn default() -> Self {
        Self::in_memory().expect("create in-memory approval store")
    }
}

impl ApprovalStore {
    pub fn in_memory() -> Result<Self, ApprovalError> {
        let connection =
            Connection::open_in_memory().map_err(|error| ApprovalError::Storage(error.to_string()))?;
        Self::from_sqlite_connection(connection)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, ApprovalError> {
        let connection =
            Connection::open(path).map_err(|error| ApprovalError::Storage(error.to_string()))?;
        Self::from_sqlite_connection(connection)
    }

    pub fn open_postgres(database_url: &str) -> Result<Self, ApprovalError> {
        let mut client = Client::connect(database_url, NoTls)
            .map_err(|error| ApprovalError::Storage(error.to_string()))?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS hosted_approvals (
                    approval_id TEXT PRIMARY KEY,
                    workspace_id TEXT NOT NULL,
                    requested_by_credential_id TEXT NOT NULL,
                    request_digest TEXT NOT NULL,
                    summary_json TEXT NOT NULL,
                    state TEXT NOT NULL,
                    requested_at BIGINT NOT NULL,
                    decided_at BIGINT,
                    decided_by_credential_id TEXT,
                    consumed_at BIGINT,
                    consumed_by_credential_id TEXT
                );
                CREATE INDEX IF NOT EXISTS hosted_approvals_workspace
                    ON hosted_approvals(workspace_id);",
            )
            .map_err(|error| ApprovalError::Storage(error.to_string()))?;
        Ok(Self {
            backend: ApprovalBackend::Postgres(Arc::new(Mutex::new(client))),
        })
    }

    fn from_sqlite_connection(connection: Connection) -> Result<Self, ApprovalError> {
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS hosted_approvals (
                    approval_id TEXT PRIMARY KEY,
                    workspace_id TEXT NOT NULL,
                    requested_by_credential_id TEXT NOT NULL,
                    request_digest TEXT NOT NULL,
                    summary_json TEXT NOT NULL,
                    state TEXT NOT NULL,
                    requested_at INTEGER NOT NULL,
                    decided_at INTEGER,
                    decided_by_credential_id TEXT,
                    consumed_at INTEGER,
                    consumed_by_credential_id TEXT
                );
                CREATE INDEX IF NOT EXISTS hosted_approvals_workspace
                    ON hosted_approvals(workspace_id);",
            )
            .map_err(|error| ApprovalError::Storage(error.to_string()))?;
        Ok(Self {
            backend: ApprovalBackend::Sqlite(Arc::new(Mutex::new(connection))),
        })
    }

    pub async fn create_async(
        &self,
        workspace_id: Uuid,
        requested_by_credential_id: Uuid,
        request: HttpExecutionRequest,
    ) -> Result<ApprovalRecord, ApprovalError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.create(workspace_id, requested_by_credential_id, &request)
        })
        .await
        .map_err(|error| ApprovalError::Storage(error.to_string()))?
    }

    pub async fn get_async(
        &self,
        workspace_id: Uuid,
        approval_id: Uuid,
    ) -> Result<Option<ApprovalRecord>, ApprovalError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.get(workspace_id, approval_id))
            .await
            .map_err(|error| ApprovalError::Storage(error.to_string()))?
    }

    pub async fn decide_async(
        &self,
        workspace_id: Uuid,
        approval_id: Uuid,
        decided_by_credential_id: Uuid,
        decision: ApprovalDecision,
    ) -> Result<ApprovalRecord, ApprovalError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.decide(
                workspace_id,
                approval_id,
                decided_by_credential_id,
                decision,
            )
        })
        .await
        .map_err(|error| ApprovalError::Storage(error.to_string()))?
    }

    pub async fn consume_async(
        &self,
        workspace_id: Uuid,
        approval_id: Uuid,
        consumed_by_credential_id: Uuid,
        request: HttpExecutionRequest,
    ) -> Result<ApprovalRecord, ApprovalError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.consume(
                workspace_id,
                approval_id,
                consumed_by_credential_id,
                &request,
            )
        })
        .await
        .map_err(|error| ApprovalError::Storage(error.to_string()))?
    }

    pub fn create(
        &self,
        workspace_id: Uuid,
        requested_by_credential_id: Uuid,
        request: &HttpExecutionRequest,
    ) -> Result<ApprovalRecord, ApprovalError> {
        let record = ApprovalRecord {
            approval_id: Uuid::now_v7(),
            workspace_id,
            requested_by_credential_id,
            request_digest: request_digest(request)?,
            summary: request_summary(request)?,
            state: ApprovalState::Pending,
            requested_at_unix_ms: unix_time_ms(),
            decided_at_unix_ms: None,
            decided_by_credential_id: None,
            consumed_at_unix_ms: None,
            consumed_by_credential_id: None,
        };
        self.insert(&record)?;
        Ok(record)
    }

    pub fn get(
        &self,
        workspace_id: Uuid,
        approval_id: Uuid,
    ) -> Result<Option<ApprovalRecord>, ApprovalError> {
        let record = match &self.backend {
            ApprovalBackend::Sqlite(connection) => connection
                .lock()
                .expect("approval store poisoned")
                .query_row(
                    "SELECT workspace_id, requested_by_credential_id, request_digest,
                            summary_json, state, requested_at, decided_at,
                            decided_by_credential_id, consumed_at, consumed_by_credential_id
                     FROM hosted_approvals WHERE approval_id=?1",
                    [approval_id.to_string()],
                    |row| {
                        Ok(StoredApprovalRow {
                            workspace_id: row.get(0)?,
                            requested_by_credential_id: row.get(1)?,
                            request_digest: row.get(2)?,
                            summary_json: row.get(3)?,
                            state: row.get(4)?,
                            requested_at: row.get(5)?,
                            decided_at: row.get(6)?,
                            decided_by_credential_id: row.get(7)?,
                            consumed_at: row.get(8)?,
                            consumed_by_credential_id: row.get(9)?,
                        })
                    },
                )
                .optional()
                .map_err(|error| ApprovalError::Storage(error.to_string()))?,
            ApprovalBackend::Postgres(client) => client
                .lock()
                .expect("approval store poisoned")
                .query_opt(
                    "SELECT workspace_id, requested_by_credential_id, request_digest,
                            summary_json, state, requested_at, decided_at,
                            decided_by_credential_id, consumed_at, consumed_by_credential_id
                     FROM hosted_approvals WHERE approval_id=$1",
                    &[&approval_id.to_string()],
                )
                .map_err(|error| ApprovalError::Storage(error.to_string()))?
                .map(|row| StoredApprovalRow {
                    workspace_id: row.get(0),
                    requested_by_credential_id: row.get(1),
                    request_digest: row.get(2),
                    summary_json: row.get(3),
                    state: row.get(4),
                    requested_at: row.get(5),
                    decided_at: row.get(6),
                    decided_by_credential_id: row.get(7),
                    consumed_at: row.get(8),
                    consumed_by_credential_id: row.get(9),
                }),
        };

        let Some(record) = record else {
            return Ok(None);
        };
        let record = record.into_record(approval_id)?;
        if record.workspace_id != workspace_id {
            return Ok(None);
        }
        Ok(Some(record))
    }

    pub fn decide(
        &self,
        workspace_id: Uuid,
        approval_id: Uuid,
        decided_by_credential_id: Uuid,
        decision: ApprovalDecision,
    ) -> Result<ApprovalRecord, ApprovalError> {
        let decided_at = unix_time_ms();
        match &self.backend {
            ApprovalBackend::Sqlite(connection) => {
                let mut connection = connection.lock().expect("approval store poisoned");
                let transaction = connection
                    .transaction()
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?;
                let record = load_sqlite_record(&transaction, approval_id)?
                    .filter(|record| record.workspace_id == workspace_id)
                    .ok_or(ApprovalError::NotFound)?;
                if record.state != ApprovalState::Pending {
                    return Err(ApprovalError::NotPending);
                }
                transaction
                    .execute(
                        "UPDATE hosted_approvals
                         SET state=?1, decided_at=?2, decided_by_credential_id=?3
                         WHERE approval_id=?4 AND workspace_id=?5 AND state='pending'",
                        params![
                            decision.state().as_str(),
                            millis_i64(decided_at)?,
                            decided_by_credential_id.to_string(),
                            approval_id.to_string(),
                            workspace_id.to_string()
                        ],
                    )
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?;
                transaction
                    .commit()
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?;
            }
            ApprovalBackend::Postgres(client) => {
                let mut client = client.lock().expect("approval store poisoned");
                let mut transaction = client
                    .transaction()
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?;
                let approval = approval_id.to_string();
                let workspace = workspace_id.to_string();
                let row = transaction
                    .query_opt(
                        "SELECT workspace_id, state FROM hosted_approvals
                         WHERE approval_id=$1 FOR UPDATE",
                        &[&approval],
                    )
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?
                    .ok_or(ApprovalError::NotFound)?;
                if row.get::<_, String>(0) != workspace {
                    return Err(ApprovalError::NotFound);
                }
                if ApprovalState::parse(row.get::<_, String>(1).as_str())?
                    != ApprovalState::Pending
                {
                    return Err(ApprovalError::NotPending);
                }
                let decided_at = millis_i64(decided_at)?;
                let decided_by = decided_by_credential_id.to_string();
                transaction
                    .execute(
                        "UPDATE hosted_approvals
                         SET state=$1, decided_at=$2, decided_by_credential_id=$3
                         WHERE approval_id=$4 AND workspace_id=$5 AND state='pending'",
                        &[
                            &decision.state().as_str(),
                            &decided_at,
                            &decided_by,
                            &approval,
                            &workspace,
                        ],
                    )
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?;
                transaction
                    .commit()
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?;
            }
        }
        self.get(workspace_id, approval_id)?
            .ok_or(ApprovalError::NotFound)
    }

    pub fn consume(
        &self,
        workspace_id: Uuid,
        approval_id: Uuid,
        consumed_by_credential_id: Uuid,
        request: &HttpExecutionRequest,
    ) -> Result<ApprovalRecord, ApprovalError> {
        let request_digest = request_digest(request)?;
        let consumed_at = unix_time_ms();

        match &self.backend {
            ApprovalBackend::Sqlite(connection) => {
                let mut connection = connection.lock().expect("approval store poisoned");
                let transaction = connection
                    .transaction()
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?;
                let record = load_sqlite_record(&transaction, approval_id)?
                    .filter(|record| record.workspace_id == workspace_id)
                    .ok_or(ApprovalError::NotFound)?;
                verify_consumable(&record, &request_digest)?;
                transaction
                    .execute(
                        "UPDATE hosted_approvals
                         SET state='consumed', consumed_at=?1, consumed_by_credential_id=?2
                         WHERE approval_id=?3 AND workspace_id=?4 AND state='approved'",
                        params![
                            millis_i64(consumed_at)?,
                            consumed_by_credential_id.to_string(),
                            approval_id.to_string(),
                            workspace_id.to_string()
                        ],
                    )
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?;
                transaction
                    .commit()
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?;
            }
            ApprovalBackend::Postgres(client) => {
                let mut client = client.lock().expect("approval store poisoned");
                let mut transaction = client
                    .transaction()
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?;
                let approval = approval_id.to_string();
                let workspace = workspace_id.to_string();
                let row = transaction
                    .query_opt(
                        "SELECT workspace_id, requested_by_credential_id, request_digest,
                                summary_json, state, requested_at, decided_at,
                                decided_by_credential_id, consumed_at, consumed_by_credential_id
                         FROM hosted_approvals WHERE approval_id=$1 FOR UPDATE",
                        &[&approval],
                    )
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?
                    .ok_or(ApprovalError::NotFound)?;
                let record = StoredApprovalRow {
                    workspace_id: row.get(0),
                    requested_by_credential_id: row.get(1),
                    request_digest: row.get(2),
                    summary_json: row.get(3),
                    state: row.get(4),
                    requested_at: row.get(5),
                    decided_at: row.get(6),
                    decided_by_credential_id: row.get(7),
                    consumed_at: row.get(8),
                    consumed_by_credential_id: row.get(9),
                }
                .into_record(approval_id)?;
                if record.workspace_id != workspace_id {
                    return Err(ApprovalError::NotFound);
                }
                verify_consumable(&record, &request_digest)?;
                let consumed_at = millis_i64(consumed_at)?;
                let consumed_by = consumed_by_credential_id.to_string();
                transaction
                    .execute(
                        "UPDATE hosted_approvals
                         SET state='consumed', consumed_at=$1, consumed_by_credential_id=$2
                         WHERE approval_id=$3 AND workspace_id=$4 AND state='approved'",
                        &[&consumed_at, &consumed_by, &approval, &workspace],
                    )
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?;
                transaction
                    .commit()
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?;
            }
        }

        self.get(workspace_id, approval_id)?
            .ok_or(ApprovalError::NotFound)
    }

    fn insert(&self, record: &ApprovalRecord) -> Result<(), ApprovalError> {
        let summary_json = serde_json::to_string(&record.summary)
            .map_err(|error| ApprovalError::Storage(error.to_string()))?;
        let requested_at = millis_i64(record.requested_at_unix_ms)?;

        match &self.backend {
            ApprovalBackend::Sqlite(connection) => {
                connection
                    .lock()
                    .expect("approval store poisoned")
                    .execute(
                        "INSERT INTO hosted_approvals
                            (approval_id, workspace_id, requested_by_credential_id,
                             request_digest, summary_json, state, requested_at)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            record.approval_id.to_string(),
                            record.workspace_id.to_string(),
                            record.requested_by_credential_id.to_string(),
                            &record.request_digest,
                            summary_json,
                            record.state.as_str(),
                            requested_at
                        ],
                    )
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?;
            }
            ApprovalBackend::Postgres(client) => {
                client
                    .lock()
                    .expect("approval store poisoned")
                    .execute(
                        "INSERT INTO hosted_approvals
                            (approval_id, workspace_id, requested_by_credential_id,
                             request_digest, summary_json, state, requested_at)
                         VALUES ($1, $2, $3, $4, $5, $6, $7)",
                        &[
                            &record.approval_id.to_string(),
                            &record.workspace_id.to_string(),
                            &record.requested_by_credential_id.to_string(),
                            &record.request_digest,
                            &summary_json,
                            &record.state.as_str(),
                            &requested_at,
                        ],
                    )
                    .map_err(|error| ApprovalError::Storage(error.to_string()))?;
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct StoredApprovalRow {
    workspace_id: String,
    requested_by_credential_id: String,
    request_digest: String,
    summary_json: String,
    state: String,
    requested_at: i64,
    decided_at: Option<i64>,
    decided_by_credential_id: Option<String>,
    consumed_at: Option<i64>,
    consumed_by_credential_id: Option<String>,
}

impl StoredApprovalRow {
    fn into_record(self, approval_id: Uuid) -> Result<ApprovalRecord, ApprovalError> {
        Ok(ApprovalRecord {
            approval_id,
            workspace_id: parse_uuid(&self.workspace_id)?,
            requested_by_credential_id: parse_uuid(&self.requested_by_credential_id)?,
            request_digest: self.request_digest,
            summary: serde_json::from_str(&self.summary_json)
                .map_err(|error| ApprovalError::Storage(error.to_string()))?,
            state: ApprovalState::parse(&self.state)?,
            requested_at_unix_ms: millis_u64(self.requested_at)?,
            decided_at_unix_ms: self.decided_at.map(millis_u64).transpose()?,
            decided_by_credential_id: self
                .decided_by_credential_id
                .map(|value| parse_uuid(&value))
                .transpose()?,
            consumed_at_unix_ms: self.consumed_at.map(millis_u64).transpose()?,
            consumed_by_credential_id: self
                .consumed_by_credential_id
                .map(|value| parse_uuid(&value))
                .transpose()?,
        })
    }
}

fn load_sqlite_record(
    transaction: &rusqlite::Transaction<'_>,
    approval_id: Uuid,
) -> Result<Option<ApprovalRecord>, ApprovalError> {
    transaction
        .query_row(
            "SELECT workspace_id, requested_by_credential_id, request_digest,
                    summary_json, state, requested_at, decided_at,
                    decided_by_credential_id, consumed_at, consumed_by_credential_id
             FROM hosted_approvals WHERE approval_id=?1",
            [approval_id.to_string()],
            |row| {
                Ok(StoredApprovalRow {
                    workspace_id: row.get(0)?,
                    requested_by_credential_id: row.get(1)?,
                    request_digest: row.get(2)?,
                    summary_json: row.get(3)?,
                    state: row.get(4)?,
                    requested_at: row.get(5)?,
                    decided_at: row.get(6)?,
                    decided_by_credential_id: row.get(7)?,
                    consumed_at: row.get(8)?,
                    consumed_by_credential_id: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(|error| ApprovalError::Storage(error.to_string()))?
        .map(|row| row.into_record(approval_id))
        .transpose()
}

fn verify_consumable(
    record: &ApprovalRecord,
    request_digest: &str,
) -> Result<(), ApprovalError> {
    match record.state {
        ApprovalState::Pending => return Err(ApprovalError::Pending),
        ApprovalState::Denied => return Err(ApprovalError::Denied),
        ApprovalState::Consumed => return Err(ApprovalError::Consumed),
        ApprovalState::Approved => {}
    }
    if record.request_digest != request_digest {
        return Err(ApprovalError::RequestMismatch);
    }
    Ok(())
}

pub fn request_digest(request: &HttpExecutionRequest) -> Result<String, ApprovalError> {
    let value =
        serde_json::to_value(request).map_err(|_| ApprovalError::InvalidRequest)?;
    let canonical = canonicalize_json(value);
    let bytes =
        serde_json::to_vec(&canonical).map_err(|_| ApprovalError::InvalidRequest)?;
    Ok(sha256_hex(&bytes))
}

pub fn request_summary(request: &HttpExecutionRequest) -> Result<ApprovalSummary, ApprovalError> {
    let url = Url::parse(&request.url).map_err(|_| ApprovalError::InvalidRequest)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(ApprovalError::InvalidRequest);
    }

    let body_sha256 = request.body.as_ref().map(|body| {
        let canonical = canonicalize_json(body.clone());
        let bytes = serde_json::to_vec(&canonical).expect("canonical JSON serializes");
        sha256_hex(&bytes)
    });

    Ok(ApprovalSummary {
        method: request.method.to_ascii_uppercase(),
        origin: url.origin().ascii_serialization(),
        path: url.path().to_owned(),
        header_names: request.headers.keys().cloned().collect(),
        secret_header_names: request.secret_headers.keys().cloned().collect(),
        body_sha256,
        capture_names: request
            .capture
            .iter()
            .map(|capture| capture.secret_name.clone())
            .collect(),
    })
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Array(values) => {
            Value::Array(values.into_iter().map(canonicalize_json).collect())
        }
        Value::Object(values) => {
            let mut entries = values.into_iter().collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut canonical = serde_json::Map::new();
            for (key, value) in entries {
                canonical.insert(key, canonicalize_json(value));
            }
            Value::Object(canonical)
        }
        value => value,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    digest(&SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn parse_uuid(value: &str) -> Result<Uuid, ApprovalError> {
    value
        .parse()
        .map_err(|error| ApprovalError::Storage(format!("invalid UUID: {error}")))
}

fn millis_i64(value: u64) -> Result<i64, ApprovalError> {
    i64::try_from(value).map_err(|error| ApprovalError::Storage(error.to_string()))
}

fn millis_u64(value: i64) -> Result<u64, ApprovalError> {
    u64::try_from(value).map_err(|error| ApprovalError::Storage(error.to_string()))
}

fn unix_time_ms() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    u64::try_from(millis).unwrap_or(u64::MAX)
}
