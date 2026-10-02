use std::{
    path::Path as FsPath,
    sync::{Arc, Mutex},
};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{
        DefaultBodyLimit, OriginalUri, Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::{
        HeaderMap, HeaderValue, Method, StatusCode,
        header::{AUTHORIZATION, COOKIE, SET_COOKIE},
    },
    response::{IntoResponse, Response},
    routing::{any, get, post},
};
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use postgres::{Client, NoTls};
use rand::RngCore;
use ring::digest::{SHA256, digest};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    hosted_identity::{ApiScope, HostedIdentityStore},
    web_assets,
};

mod native_realtime;
mod ports;

use native_realtime::TokioAnonymousInteractionStream;
use ports::{
    AnonymousExposureRepository, AnonymousInteractionStream, CaptureAnonymousInteraction,
    CreateAnonymousExposure, InteractionStreamError, StoredInteraction,
};

pub const ANONYMOUS_TTL_SECONDS: u64 = 5 * 24 * 60 * 60;
pub const ANONYMOUS_REQUEST_LIMIT: u32 = 100;
pub const ANONYMOUS_ACTIVE_LIMIT: u32 = 3;
pub const ANONYMOUS_MAX_BODY_BYTES: usize = 5 * 1024 * 1024;
pub const ANONYMOUS_MAX_RETAINED_BYTES: u64 = 50 * 1024 * 1024;

const PRINCIPAL_COOKIE: &str = "hooktry_anon";
const PRINCIPAL_HEADER: &str = "x-hooktry-anonymous-principal";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnonymousExposureSummary {
    pub exposure_id: Uuid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<Uuid>,
    pub created_at_unix_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_unix_seconds: Option<u64>,
    pub request_count: u32,
    pub retained_bytes: u64,
    pub request_limit: u32,
    pub max_body_bytes: usize,
    pub max_retained_bytes: u64,
    pub claimed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnonymousProvision {
    #[serde(flatten)]
    pub exposure: AnonymousExposureSummary,
    pub hook_url: String,
    pub view_url: String,
    pub view_websocket_url: String,
    pub claim_url: String,
    pub anonymous_principal: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnonymousIngressProvision {
    pub exposure_id: Uuid,
    pub hook_url: String,
    pub expires_at_unix_seconds: Option<u64>,
    pub request_limit: u32,
    pub max_body_bytes: usize,
    pub max_retained_bytes: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AnonymousInteraction {
    pub interaction_id: Uuid,
    pub exposure_id: Uuid,
    pub sequence: u32,
    pub received_at_unix_ms: u64,
    pub method: String,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body_encoding: &'static str,
    pub body: String,
    pub body_bytes: usize,
}

impl StoredInteraction {
    fn wire(&self) -> AnonymousInteraction {
        let (body_encoding, body) = match std::str::from_utf8(&self.body) {
            Ok(text) => ("utf8", text.to_owned()),
            Err(_) => ("hex", hex_encode(&self.body)),
        };
        AnonymousInteraction {
            interaction_id: self.interaction_id,
            exposure_id: self.exposure_id,
            sequence: self.sequence,
            received_at_unix_ms: self.received_at_unix_ms,
            method: self.method.clone(),
            path: self.path.clone(),
            query: self.query.clone(),
            headers: self.headers.clone(),
            body_encoding,
            body,
            body_bytes: self.body.len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnonymousError {
    ActiveLimit,
    NotFound,
    Expired,
    RequestLimit,
    ByteLimit,
    BodyTooLarge,
    InvalidClaim,
    Storage(String),
}

#[derive(Clone)]
enum AnonymousBackend {
    Sqlite(Arc<Mutex<Connection>>),
    Postgres(Arc<Mutex<Client>>),
}

#[derive(Clone)]
pub struct AnonymousExposureStore {
    backend: AnonymousBackend,
}

impl Default for AnonymousExposureStore {
    fn default() -> Self {
        Self::in_memory().expect("create in-memory anonymous exposure store")
    }
}

impl AnonymousExposureStore {
    pub fn in_memory() -> Result<Self, AnonymousError> {
        let connection = Connection::open_in_memory()
            .map_err(|error| AnonymousError::Storage(error.to_string()))?;
        Self::from_sqlite(connection)
    }

    pub fn open(path: impl AsRef<FsPath>) -> Result<Self, AnonymousError> {
        let connection =
            Connection::open(path).map_err(|error| AnonymousError::Storage(error.to_string()))?;
        Self::from_sqlite(connection)
    }

    pub fn open_postgres(database_url: &str) -> Result<Self, AnonymousError> {
        let mut client = Client::connect(database_url, NoTls)
            .map_err(|error| AnonymousError::Storage(error.to_string()))?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS anonymous_exposures (
                    exposure_id TEXT PRIMARY KEY,
                    principal_digest BYTEA NOT NULL,
                    ingress_capability_digest BYTEA NOT NULL UNIQUE,
                    view_capability_digest BYTEA NOT NULL UNIQUE,
                    claim_capability_digest BYTEA UNIQUE,
                    workspace_id TEXT,
                    created_at BIGINT NOT NULL,
                    expires_at BIGINT NOT NULL,
                    request_count INTEGER NOT NULL DEFAULT 0,
                    retained_bytes BIGINT NOT NULL DEFAULT 0
                );
                CREATE INDEX IF NOT EXISTS anonymous_exposures_principal
                    ON anonymous_exposures(principal_digest);
                CREATE TABLE IF NOT EXISTS anonymous_interactions (
                    interaction_id TEXT PRIMARY KEY,
                    exposure_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL,
                    received_at_ms BIGINT NOT NULL,
                    method TEXT NOT NULL,
                    path TEXT NOT NULL,
                    query TEXT,
                    headers_json TEXT NOT NULL,
                    body BYTEA NOT NULL,
                    UNIQUE(exposure_id, sequence)
                );
                CREATE INDEX IF NOT EXISTS anonymous_interactions_exposure
                    ON anonymous_interactions(exposure_id, sequence);",
            )
            .map_err(|error| AnonymousError::Storage(error.to_string()))?;
        Ok(Self {
            backend: AnonymousBackend::Postgres(Arc::new(Mutex::new(client))),
        })
    }

    fn from_sqlite(connection: Connection) -> Result<Self, AnonymousError> {
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS anonymous_exposures (
                    exposure_id TEXT PRIMARY KEY,
                    principal_digest BLOB NOT NULL,
                    ingress_capability_digest BLOB NOT NULL UNIQUE,
                    view_capability_digest BLOB NOT NULL UNIQUE,
                    claim_capability_digest BLOB UNIQUE,
                    workspace_id TEXT,
                    created_at INTEGER NOT NULL,
                    expires_at INTEGER NOT NULL,
                    request_count INTEGER NOT NULL DEFAULT 0,
                    retained_bytes INTEGER NOT NULL DEFAULT 0
                );
                CREATE INDEX IF NOT EXISTS anonymous_exposures_principal
                    ON anonymous_exposures(principal_digest);
                CREATE TABLE IF NOT EXISTS anonymous_interactions (
                    interaction_id TEXT PRIMARY KEY,
                    exposure_id TEXT NOT NULL,
                    sequence INTEGER NOT NULL,
                    received_at_ms INTEGER NOT NULL,
                    method TEXT NOT NULL,
                    path TEXT NOT NULL,
                    query TEXT,
                    headers_json TEXT NOT NULL,
                    body BLOB NOT NULL,
                    UNIQUE(exposure_id, sequence)
                );
                CREATE INDEX IF NOT EXISTS anonymous_interactions_exposure
                    ON anonymous_interactions(exposure_id, sequence);",
            )
            .map_err(|error| AnonymousError::Storage(error.to_string()))?;
        Ok(Self {
            backend: AnonymousBackend::Sqlite(Arc::new(Mutex::new(connection))),
        })
    }

    pub async fn purge_expired_async(&self) -> Result<(), AnonymousError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.purge_expired(unix_seconds_now()))
            .await
            .map_err(|error| AnonymousError::Storage(error.to_string()))?
    }

    fn purge_expired(&self, now: u64) -> Result<(), AnonymousError> {
        let now = i64_from_u64(now)?;
        match &self.backend {
            AnonymousBackend::Sqlite(connection) => {
                let mut connection = connection.lock().expect("anonymous store poisoned");
                let tx = connection
                    .transaction()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                purge_sqlite(&tx, now)?;
                tx.commit()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))
            }
            AnonymousBackend::Postgres(client) => {
                let mut client = client.lock().expect("anonymous store poisoned");
                let mut tx = client
                    .transaction()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                purge_postgres(&mut tx, now)?;
                tx.commit()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))
            }
        }
    }

    async fn create_async(
        &self,
        input: CreateAnonymousExposure,
    ) -> Result<AnonymousExposureSummary, AnonymousError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.create(input))
            .await
            .map_err(|error| AnonymousError::Storage(error.to_string()))?
    }

    fn create(
        &self,
        input: CreateAnonymousExposure,
    ) -> Result<AnonymousExposureSummary, AnonymousError> {
        let CreateAnonymousExposure {
            principal_digest,
            ingress_capability_digest,
            view_capability_digest,
            claim_capability_digest,
            exposure_id,
            now,
            expires_at,
        } = input;
        let now_i64 = i64_from_u64(now)?;
        let expires_i64 = i64_from_u64(expires_at)?;
        match &self.backend {
            AnonymousBackend::Sqlite(connection) => {
                let mut connection = connection.lock().expect("anonymous store poisoned");
                let tx = connection
                    .transaction()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                purge_sqlite(&tx, now_i64)?;
                let active: i64 = tx
                    .query_row(
                        "SELECT COUNT(*) FROM anonymous_exposures
                         WHERE principal_digest = ?1
                           AND workspace_id IS NULL
                           AND expires_at > ?2",
                        params![principal_digest.as_slice(), now_i64],
                        |row| row.get(0),
                    )
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                if active >= i64::from(ANONYMOUS_ACTIVE_LIMIT) {
                    return Err(AnonymousError::ActiveLimit);
                }
                tx.execute(
                    "INSERT INTO anonymous_exposures (
                        exposure_id, principal_digest, ingress_capability_digest, view_capability_digest, claim_capability_digest,
                        workspace_id, created_at, expires_at, request_count, retained_bytes
                     ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, 0, 0)",
                    params![
                        exposure_id.to_string(),
                        principal_digest.as_slice(),
                        ingress_capability_digest.as_slice(),
                        view_capability_digest.as_slice(),
                        claim_capability_digest.as_slice(),
                        now_i64,
                        expires_i64,
                    ],
                )
                .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                tx.commit()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
            }
            AnonymousBackend::Postgres(client) => {
                let mut client = client.lock().expect("anonymous store poisoned");
                let mut tx = client
                    .transaction()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                let principal_lock = advisory_lock_key(&principal_digest);
                tx.query_one("SELECT pg_advisory_xact_lock($1)", &[&principal_lock])
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                purge_postgres(&mut tx, now_i64)?;
                let row = tx
                    .query_one(
                        "SELECT COUNT(*) FROM anonymous_exposures
                         WHERE principal_digest = $1
                           AND workspace_id IS NULL
                           AND expires_at > $2",
                        &[&principal_digest.as_slice(), &now_i64],
                    )
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                let active: i64 = row.get(0);
                if active >= i64::from(ANONYMOUS_ACTIVE_LIMIT) {
                    return Err(AnonymousError::ActiveLimit);
                }
                let exposure_id_text = exposure_id.to_string();
                tx.execute(
                    "INSERT INTO anonymous_exposures (
                        exposure_id, principal_digest, ingress_capability_digest, view_capability_digest, claim_capability_digest,
                        workspace_id, created_at, expires_at, request_count, retained_bytes
                     ) VALUES ($1, $2, $3, $4, $5, NULL, $6, $7, 0, 0)",
                    &[
                        &exposure_id_text,
                        &principal_digest.as_slice(),
                        &ingress_capability_digest.as_slice(),
                        &view_capability_digest.as_slice(),
                        &claim_capability_digest.as_slice(),
                        &now_i64,
                        &expires_i64,
                    ],
                )
                .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                tx.commit()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
            }
        }

        Ok(summary(exposure_id, None, now, Some(expires_at), 0, 0))
    }

    async fn capture_async(
        &self,
        input: CaptureAnonymousInteraction,
    ) -> Result<StoredInteraction, AnonymousError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.capture(input))
            .await
            .map_err(|error| AnonymousError::Storage(error.to_string()))?
    }

    fn capture(
        &self,
        input: CaptureAnonymousInteraction,
    ) -> Result<StoredInteraction, AnonymousError> {
        let CaptureAnonymousInteraction {
            ingress_capability_digest,
            received_at_ms,
            method,
            path,
            query,
            headers,
            body,
        } = input;
        if body.len() > ANONYMOUS_MAX_BODY_BYTES {
            return Err(AnonymousError::BodyTooLarge);
        }

        let now_seconds = received_at_ms / 1000;
        let now_i64 = i64_from_u64(now_seconds)?;
        let received_at_i64 = i64_from_u64(received_at_ms)?;
        let body_len = u64::try_from(body.len())
            .map_err(|error| AnonymousError::Storage(error.to_string()))?;
        let interaction_id = Uuid::now_v7();
        let headers_json = serde_json::to_string(&headers)
            .map_err(|error| AnonymousError::Storage(error.to_string()))?;

        let (exposure_id, sequence) = match &self.backend {
            AnonymousBackend::Sqlite(connection) => {
                let mut connection = connection.lock().expect("anonymous store poisoned");
                let tx = connection
                    .transaction()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                let raw = tx
                    .query_row(
                        "SELECT exposure_id, workspace_id, expires_at, request_count, retained_bytes
                         FROM anonymous_exposures WHERE ingress_capability_digest = ?1",
                        params![ingress_capability_digest.as_slice()],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, Option<String>>(1)?,
                                row.get::<_, i64>(2)?,
                                row.get::<_, i64>(3)?,
                                row.get::<_, i64>(4)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?
                    .ok_or(AnonymousError::NotFound)?;
                let exposure_id: Uuid = raw.0.parse().map_err(|error| {
                    AnonymousError::Storage(format!("invalid exposure id: {error}"))
                })?;
                check_capture_policy(raw.1.is_some(), raw.2, raw.3, raw.4, now_i64, body_len)?;
                let sequence = u32::try_from(raw.3 + 1)
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                tx.execute(
                    "INSERT INTO anonymous_interactions (
                        interaction_id, exposure_id, sequence, received_at_ms, method, path, query,
                        headers_json, body
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    params![
                        interaction_id.to_string(),
                        exposure_id.to_string(),
                        i64::from(sequence),
                        received_at_i64,
                        &method,
                        &path,
                        &query,
                        &headers_json,
                        &body,
                    ],
                )
                .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                tx.execute(
                    "UPDATE anonymous_exposures
                     SET request_count = request_count + 1,
                         retained_bytes = retained_bytes + ?1
                     WHERE exposure_id = ?2",
                    params![i64_from_u64(body_len)?, exposure_id.to_string()],
                )
                .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                tx.commit()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                (exposure_id, sequence)
            }
            AnonymousBackend::Postgres(client) => {
                let mut client = client.lock().expect("anonymous store poisoned");
                let mut tx = client
                    .transaction()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                let row = tx
                    .query_opt(
                        "SELECT exposure_id, workspace_id, expires_at, request_count, retained_bytes
                         FROM anonymous_exposures
                         WHERE ingress_capability_digest = $1
                         FOR UPDATE",
                        &[&ingress_capability_digest.as_slice()],
                    )
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?
                    .ok_or(AnonymousError::NotFound)?;
                let exposure_id_text: String = row.get(0);
                let workspace_id: Option<String> = row.get(1);
                let expires_at: i64 = row.get(2);
                let request_count: i32 = row.get(3);
                let retained_bytes: i64 = row.get(4);
                check_capture_policy(
                    workspace_id.is_some(),
                    expires_at,
                    i64::from(request_count),
                    retained_bytes,
                    now_i64,
                    body_len,
                )?;
                let exposure_id: Uuid = exposure_id_text.parse().map_err(|error| {
                    AnonymousError::Storage(format!("invalid exposure id: {error}"))
                })?;
                let sequence = u32::try_from(i64::from(request_count) + 1)
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                let sequence_i32 = i32::try_from(sequence)
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                tx.execute(
                    "INSERT INTO anonymous_interactions (
                        interaction_id, exposure_id, sequence, received_at_ms, method, path, query,
                        headers_json, body
                     ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                    &[
                        &interaction_id.to_string(),
                        &exposure_id_text,
                        &sequence_i32,
                        &received_at_i64,
                        &method,
                        &path,
                        &query,
                        &headers_json,
                        &body,
                    ],
                )
                .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                let body_len_i64 = i64_from_u64(body_len)?;
                tx.execute(
                    "UPDATE anonymous_exposures
                     SET request_count = request_count + 1,
                         retained_bytes = retained_bytes + $1
                     WHERE exposure_id = $2",
                    &[&body_len_i64, &exposure_id_text],
                )
                .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                tx.commit()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                (exposure_id, sequence)
            }
        };

        Ok(StoredInteraction {
            interaction_id,
            exposure_id,
            sequence,
            received_at_unix_ms: received_at_ms,
            method,
            path,
            query,
            headers,
            body,
        })
    }

    async fn view_async(
        &self,
        view_capability_digest: [u8; 32],
        now: u64,
    ) -> Result<AnonymousExposureSummary, AnonymousError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.view(view_capability_digest, now))
            .await
            .map_err(|error| AnonymousError::Storage(error.to_string()))?
    }

    fn view(
        &self,
        view_capability_digest: [u8; 32],
        now: u64,
    ) -> Result<AnonymousExposureSummary, AnonymousError> {
        let now_i64 = i64_from_u64(now)?;
        match &self.backend {
            AnonymousBackend::Sqlite(connection) => {
                let raw = connection
                    .lock()
                    .expect("anonymous store poisoned")
                    .query_row(
                        "SELECT exposure_id, workspace_id, created_at, expires_at, request_count, retained_bytes
                         FROM anonymous_exposures WHERE view_capability_digest = ?1",
                        params![view_capability_digest.as_slice()],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, Option<String>>(1)?,
                                row.get::<_, i64>(2)?,
                                row.get::<_, i64>(3)?,
                                row.get::<_, i64>(4)?,
                                row.get::<_, i64>(5)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?
                    .ok_or(AnonymousError::NotFound)?;
                summary_from_raw(raw, now_i64)
            }
            AnonymousBackend::Postgres(client) => {
                let row = client
                    .lock()
                    .expect("anonymous store poisoned")
                    .query_opt(
                        "SELECT exposure_id, workspace_id, created_at, expires_at, request_count, retained_bytes
                         FROM anonymous_exposures WHERE view_capability_digest = $1",
                        &[&view_capability_digest.as_slice()],
                    )
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?
                    .ok_or(AnonymousError::NotFound)?;
                summary_from_raw(
                    (
                        row.get::<_, String>(0),
                        row.get::<_, Option<String>>(1),
                        row.get::<_, i64>(2),
                        row.get::<_, i64>(3),
                        i64::from(row.get::<_, i32>(4)),
                        row.get::<_, i64>(5),
                    ),
                    now_i64,
                )
            }
        }
    }

    async fn interactions_async(
        &self,
        exposure_id: Uuid,
    ) -> Result<Vec<StoredInteraction>, AnonymousError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.interactions(exposure_id))
            .await
            .map_err(|error| AnonymousError::Storage(error.to_string()))?
    }

    fn interactions(&self, exposure_id: Uuid) -> Result<Vec<StoredInteraction>, AnonymousError> {
        let exposure_id_text = exposure_id.to_string();
        match &self.backend {
            AnonymousBackend::Sqlite(connection) => {
                let connection = connection.lock().expect("anonymous store poisoned");
                let mut statement = connection
                    .prepare(
                        "SELECT interaction_id, sequence, received_at_ms, method, path, query,
                                headers_json, body
                         FROM anonymous_interactions
                         WHERE exposure_id = ?1
                         ORDER BY sequence",
                    )
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                let rows = statement
                    .query_map([&exposure_id_text], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, Option<String>>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, Vec<u8>>(7)?,
                        ))
                    })
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                rows.map(|row| {
                    interaction_from_raw(
                        exposure_id,
                        row.map_err(|error| AnonymousError::Storage(error.to_string()))?,
                    )
                })
                .collect()
            }
            AnonymousBackend::Postgres(client) => {
                let rows = client
                    .lock()
                    .expect("anonymous store poisoned")
                    .query(
                        "SELECT interaction_id, sequence, received_at_ms, method, path, query,
                                headers_json, body
                         FROM anonymous_interactions
                         WHERE exposure_id = $1
                         ORDER BY sequence",
                        &[&exposure_id_text],
                    )
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                rows.into_iter()
                    .map(|row| {
                        interaction_from_raw(
                            exposure_id,
                            (
                                row.get::<_, String>(0),
                                i64::from(row.get::<_, i32>(1)),
                                row.get::<_, i64>(2),
                                row.get::<_, String>(3),
                                row.get::<_, String>(4),
                                row.get::<_, Option<String>>(5),
                                row.get::<_, String>(6),
                                row.get::<_, Vec<u8>>(7),
                            ),
                        )
                    })
                    .collect()
            }
        }
    }

    async fn claim_async(
        &self,
        claim_capability_digest: [u8; 32],
        workspace_id: Uuid,
        now: u64,
    ) -> Result<AnonymousExposureSummary, AnonymousError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.claim(claim_capability_digest, workspace_id, now))
            .await
            .map_err(|error| AnonymousError::Storage(error.to_string()))?
    }

    fn claim(
        &self,
        claim_capability_digest: [u8; 32],
        workspace_id: Uuid,
        now: u64,
    ) -> Result<AnonymousExposureSummary, AnonymousError> {
        let now_i64 = i64_from_u64(now)?;
        let workspace_id_text = workspace_id.to_string();
        match &self.backend {
            AnonymousBackend::Sqlite(connection) => {
                let mut connection = connection.lock().expect("anonymous store poisoned");
                let tx = connection
                    .transaction()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                let raw = tx
                    .query_row(
                        "SELECT exposure_id, workspace_id, created_at, expires_at, request_count, retained_bytes
                         FROM anonymous_exposures WHERE claim_capability_digest = ?1",
                        params![claim_capability_digest.as_slice()],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, Option<String>>(1)?,
                                row.get::<_, i64>(2)?,
                                row.get::<_, i64>(3)?,
                                row.get::<_, i64>(4)?,
                                row.get::<_, i64>(5)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?
                    .ok_or(AnonymousError::InvalidClaim)?;
                if raw.1.is_some() || now_i64 >= raw.3 {
                    return Err(AnonymousError::InvalidClaim);
                }
                tx.execute(
                    "UPDATE anonymous_exposures
                     SET workspace_id = ?1, claim_capability_digest = NULL
                     WHERE exposure_id = ?2 AND claim_capability_digest = ?3",
                    params![
                        &workspace_id_text,
                        &raw.0,
                        claim_capability_digest.as_slice()
                    ],
                )
                .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                tx.commit()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                summary_from_raw(
                    (raw.0, Some(workspace_id_text), raw.2, raw.3, raw.4, raw.5),
                    now_i64,
                )
            }
            AnonymousBackend::Postgres(client) => {
                let mut client = client.lock().expect("anonymous store poisoned");
                let mut tx = client
                    .transaction()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                let row = tx
                    .query_opt(
                        "SELECT exposure_id, workspace_id, created_at, expires_at, request_count, retained_bytes
                         FROM anonymous_exposures
                         WHERE claim_capability_digest = $1
                         FOR UPDATE",
                        &[&claim_capability_digest.as_slice()],
                    )
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?
                    .ok_or(AnonymousError::InvalidClaim)?;
                let exposure_id: String = row.get(0);
                let current_workspace: Option<String> = row.get(1);
                let created_at: i64 = row.get(2);
                let expires_at: i64 = row.get(3);
                let request_count: i32 = row.get(4);
                let retained_bytes: i64 = row.get(5);
                if current_workspace.is_some() || now_i64 >= expires_at {
                    return Err(AnonymousError::InvalidClaim);
                }
                tx.execute(
                    "UPDATE anonymous_exposures
                     SET workspace_id = $1, claim_capability_digest = NULL
                     WHERE exposure_id = $2 AND claim_capability_digest = $3",
                    &[
                        &workspace_id_text,
                        &exposure_id,
                        &claim_capability_digest.as_slice(),
                    ],
                )
                .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                tx.commit()
                    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                summary_from_raw(
                    (
                        exposure_id,
                        Some(workspace_id_text),
                        created_at,
                        expires_at,
                        i64::from(request_count),
                        retained_bytes,
                    ),
                    now_i64,
                )
            }
        }
    }
}

impl AnonymousExposureRepository for AnonymousExposureStore {
    fn purge_expired(&self) -> ports::PortFuture<'_, Result<(), AnonymousError>> {
        Box::pin(self.purge_expired_async())
    }

    fn create(
        &self,
        input: CreateAnonymousExposure,
    ) -> ports::PortFuture<'_, Result<AnonymousExposureSummary, AnonymousError>> {
        Box::pin(self.create_async(input))
    }

    fn capture(
        &self,
        input: CaptureAnonymousInteraction,
    ) -> ports::PortFuture<'_, Result<StoredInteraction, AnonymousError>> {
        Box::pin(self.capture_async(input))
    }

    fn view(
        &self,
        view_capability_digest: [u8; 32],
        now: u64,
    ) -> ports::PortFuture<'_, Result<AnonymousExposureSummary, AnonymousError>> {
        Box::pin(self.view_async(view_capability_digest, now))
    }

    fn interactions(
        &self,
        exposure_id: Uuid,
    ) -> ports::PortFuture<'_, Result<Vec<StoredInteraction>, AnonymousError>> {
        Box::pin(self.interactions_async(exposure_id))
    }

    fn claim(
        &self,
        claim_capability_digest: [u8; 32],
        workspace_id: Uuid,
        now: u64,
    ) -> ports::PortFuture<'_, Result<AnonymousExposureSummary, AnonymousError>> {
        Box::pin(self.claim_async(claim_capability_digest, workspace_id, now))
    }
}

#[derive(Clone)]
pub struct AnonymousExposureService {
    store: Arc<dyn AnonymousExposureRepository>,
    stream: Arc<dyn AnonymousInteractionStream>,
    public_base_url: String,
    viewer_ws_base_url: String,
}

impl AnonymousExposureService {
    pub fn new(store: AnonymousExposureStore, public_base_url: impl Into<String>) -> Self {
        Self::with_ports(
            Arc::new(store),
            Arc::new(TokioAnonymousInteractionStream::default()),
            public_base_url,
        )
    }

    fn with_ports(
        store: Arc<dyn AnonymousExposureRepository>,
        stream: Arc<dyn AnonymousInteractionStream>,
        public_base_url: impl Into<String>,
    ) -> Self {
        let public_base_url = public_base_url.into().trim_end_matches('/').to_owned();
        let viewer_ws_base_url = websocket_base_url(&public_base_url);
        Self {
            store,
            stream,
            public_base_url,
            viewer_ws_base_url,
        }
    }

    pub fn with_store(&self, store: AnonymousExposureStore) -> Self {
        Self {
            store: Arc::new(store),
            stream: self.stream.clone(),
            public_base_url: self.public_base_url.clone(),
            viewer_ws_base_url: self.viewer_ws_base_url.clone(),
        }
    }

    pub async fn purge_expired(&self) -> Result<(), AnonymousError> {
        self.store.purge_expired().await
    }

    pub async fn provision_ingress(&self) -> Result<AnonymousIngressProvision, AnonymousError> {
        let provision = self.provision(None).await?;
        Ok(AnonymousIngressProvision {
            exposure_id: provision.exposure.exposure_id,
            hook_url: provision.hook_url,
            expires_at_unix_seconds: provision.exposure.expires_at_unix_seconds,
            request_limit: provision.exposure.request_limit,
            max_body_bytes: provision.exposure.max_body_bytes,
            max_retained_bytes: provision.exposure.max_retained_bytes,
        })
    }

    async fn provision(
        &self,
        principal: Option<String>,
    ) -> Result<AnonymousProvision, AnonymousError> {
        let principal = principal.unwrap_or_else(random_principal);
        let hook_token = random_capability("hk_");
        let view_token = random_capability("vw_");
        let claim_token = random_capability("cl_");
        let exposure_id = Uuid::now_v7();
        let now = unix_seconds_now();
        let expires_at = now + ANONYMOUS_TTL_SECONDS;
        let exposure = self
            .store
            .create(CreateAnonymousExposure {
                principal_digest: token_digest(&principal),
                ingress_capability_digest: token_digest(&hook_token),
                view_capability_digest: token_digest(&view_token),
                claim_capability_digest: token_digest(&claim_token),
                exposure_id,
                now,
                expires_at,
            })
            .await?;

        Ok(AnonymousProvision {
            exposure,
            hook_url: format!("{}/hook/{hook_token}", self.public_base_url),
            view_url: format!("{}/view/{view_token}", self.public_base_url),
            view_websocket_url: format!("{}/view/{view_token}", self.viewer_ws_base_url),
            claim_url: format!("{}/claim/{claim_token}", self.public_base_url),
            anonymous_principal: principal,
        })
    }

    async fn capture(
        &self,
        ingress_capability_token: &str,
        method: String,
        path: String,
        query: Option<String>,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    ) -> Result<StoredInteraction, AnonymousError> {
        let interaction = self
            .store
            .capture(CaptureAnonymousInteraction {
                ingress_capability_digest: token_digest(ingress_capability_token),
                received_at_ms: unix_millis_now(),
                method,
                path,
                query,
                headers,
                body,
            })
            .await?;
        self.stream.publish(interaction.clone());
        Ok(interaction)
    }

    async fn view(
        &self,
        view_capability_token: &str,
    ) -> Result<AnonymousExposureSummary, AnonymousError> {
        self.store
            .view(token_digest(view_capability_token), unix_seconds_now())
            .await
    }

    async fn backlog(&self, exposure_id: Uuid) -> Result<Vec<StoredInteraction>, AnonymousError> {
        self.store.interactions(exposure_id).await
    }

    async fn claim(
        &self,
        claim_capability_token: &str,
        workspace_id: Uuid,
    ) -> Result<AnonymousExposureSummary, AnonymousError> {
        self.store
            .claim(
                token_digest(claim_capability_token),
                workspace_id,
                unix_seconds_now(),
            )
            .await
    }

    fn subscribe(&self, exposure_id: Uuid) -> Box<dyn ports::AnonymousInteractionSubscription> {
        self.stream.subscribe(exposure_id)
    }
}

#[derive(Clone)]
struct AnonymousRouterState {
    anonymous: AnonymousExposureService,
    identities: HostedIdentityStore,
}

pub fn anonymous_app(
    anonymous: AnonymousExposureService,
    identities: HostedIdentityStore,
) -> Router {
    let state = AnonymousRouterState {
        anonymous,
        identities,
    };
    Router::new()
        .route("/api/v1/hooks", post(create_hook))
        .route("/view/{view_token}", get(view_anonymous))
        .route("/claim/{claim_token}", post(claim_anonymous))
        .route("/hook/{hook_token}", any(hook_root))
        .route("/hook/{hook_token}/{*path}", any(hook_path))
        .layer(DefaultBodyLimit::max(ANONYMOUS_MAX_BODY_BYTES))
        .with_state(state)
}

async fn create_hook(
    State(state): State<AnonymousRouterState>,
    headers: HeaderMap,
) -> Result<Response, AnonymousApiError> {
    let existing_principal = anonymous_principal(&headers);
    let provision = state
        .anonymous
        .provision(existing_principal)
        .await
        .map_err(AnonymousApiError::from)?;

    let cookie = format!(
        "{PRINCIPAL_COOKIE}={}; Max-Age={ANONYMOUS_TTL_SECONDS}; Path=/; HttpOnly; SameSite=Lax",
        provision.anonymous_principal
    );
    let mut response = (StatusCode::CREATED, Json(&provision)).into_response();
    response.headers_mut().insert(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| AnonymousApiError::internal())?,
    );
    Ok(response)
}

async fn hook_root(
    State(state): State<AnonymousRouterState>,
    Path(hook_token): Path<String>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, AnonymousApiError> {
    capture_request(
        state,
        hook_token,
        "/".to_owned(),
        uri.query().map(ToOwned::to_owned),
        method,
        headers,
        body,
    )
    .await
}

async fn hook_path(
    State(state): State<AnonymousRouterState>,
    Path((hook_token, path)): Path<(String, String)>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, AnonymousApiError> {
    capture_request(
        state,
        hook_token,
        format!("/{path}"),
        uri.query().map(ToOwned::to_owned),
        method,
        headers,
        body,
    )
    .await
}

async fn capture_request(
    state: AnonymousRouterState,
    hook_token: String,
    path: String,
    query: Option<String>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, AnonymousApiError> {
    if body.len() > ANONYMOUS_MAX_BODY_BYTES {
        return Err(AnonymousApiError::from(AnonymousError::BodyTooLarge));
    }
    let interaction = state
        .anonymous
        .capture(
            &hook_token,
            method.as_str().to_owned(),
            path,
            query,
            capture_headers(&headers),
            body.to_vec(),
        )
        .await
        .map_err(AnonymousApiError::from)?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "ok": true,
            "interaction_id": interaction.interaction_id,
            "sequence": interaction.sequence
        })),
    ))
}

async fn view_anonymous(
    State(state): State<AnonymousRouterState>,
    Path(view_token): Path<String>,
    ws: Result<WebSocketUpgrade, axum::extract::ws::rejection::WebSocketUpgradeRejection>,
) -> Result<Response, AnonymousApiError> {
    let exposure = state
        .anonymous
        .view(&view_token)
        .await
        .map_err(AnonymousApiError::from)?;

    if let Ok(ws) = ws {
        let service = state.anonymous.clone();
        return Ok(ws
            .on_upgrade(move |socket| async move {
                let _ = serve_viewer(socket, service, exposure).await;
            })
            .into_response());
    }

    Ok(web_assets::index_response())
}

async fn serve_viewer(
    socket: WebSocket,
    service: AnonymousExposureService,
    exposure: AnonymousExposureSummary,
) -> Result<(), AnonymousError> {
    let mut live = service.subscribe(exposure.exposure_id);
    let backlog = service.backlog(exposure.exposure_id).await?;
    let mut last_sequence = backlog.last().map(|item| item.sequence).unwrap_or(0);
    let (mut writer, mut reader) = socket.split();

    send_frame(
        &mut writer,
        serde_json::json!({
            "type": "ready",
            "exposure": exposure
        }),
    )
    .await?;

    for interaction in backlog {
        send_frame(
            &mut writer,
            serde_json::json!({
                "type": "interaction",
                "interaction": interaction.wire()
            }),
        )
        .await?;
    }

    loop {
        tokio::select! {
            incoming = reader.next() => {
                match incoming {
                    Some(Ok(Message::Ping(data))) => {
                        writer
                            .send(Message::Pong(data))
                            .await
                            .map_err(|error| AnonymousError::Storage(error.to_string()))?;
                    }
                    Some(Ok(Message::Close(_))) | None => return Ok(()),
                    Some(Ok(_)) => {}
                    Some(Err(error)) => return Err(AnonymousError::Storage(error.to_string())),
                }
            }
            event = live.recv() => {
                match event {
                    Ok(interaction) => {
                        if interaction.sequence <= last_sequence {
                            continue;
                        }
                        last_sequence = interaction.sequence;
                        send_frame(
                            &mut writer,
                            serde_json::json!({
                                "type": "interaction",
                                "interaction": interaction.wire()
                            }),
                        ).await?;
                    }
                    Err(InteractionStreamError::Lagged) => {
                        send_frame(
                            &mut writer,
                            serde_json::json!({
                                "type": "resync_required"
                            }),
                        ).await?;
                        return Ok(());
                    }
                    Err(InteractionStreamError::Closed) => return Ok(()),
                }
            }
        }
    }
}

async fn send_frame(
    writer: &mut SplitSink<WebSocket, Message>,
    value: serde_json::Value,
) -> Result<(), AnonymousError> {
    let payload = serde_json::to_string(&value)
        .map_err(|error| AnonymousError::Storage(error.to_string()))?;
    writer
        .send(Message::Text(payload.into()))
        .await
        .map_err(|error| AnonymousError::Storage(error.to_string()))
}

async fn claim_anonymous(
    State(state): State<AnonymousRouterState>,
    Path(claim_token): Path<String>,
    headers: HeaderMap,
) -> Result<Json<AnonymousExposureSummary>, AnonymousApiError> {
    let token = bearer_token(&headers)
        .ok_or_else(AnonymousApiError::unauthorized)?
        .to_owned();
    let authorization = state
        .identities
        .authorize_async(token, ApiScope::ExposuresCreate)
        .await
        .map_err(|_| AnonymousApiError::unauthorized())?;
    let exposure = state
        .anonymous
        .claim(&claim_token, authorization.workspace_id)
        .await
        .map_err(AnonymousApiError::from)?;
    Ok(Json(exposure))
}

#[derive(Debug)]
struct AnonymousApiError {
    status: StatusCode,
    code: &'static str,
}

impl AnonymousApiError {
    fn new(status: StatusCode, code: &'static str) -> Self {
        Self { status, code }
    }

    fn internal() -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
    }

    fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized")
    }
}

impl From<AnonymousError> for AnonymousApiError {
    fn from(value: AnonymousError) -> Self {
        match value {
            AnonymousError::ActiveLimit => Self::new(StatusCode::TOO_MANY_REQUESTS, "active_limit"),
            AnonymousError::NotFound => Self::new(StatusCode::NOT_FOUND, "not_found"),
            AnonymousError::Expired => Self::new(StatusCode::GONE, "expired"),
            AnonymousError::RequestLimit => {
                Self::new(StatusCode::TOO_MANY_REQUESTS, "request_limit")
            }
            AnonymousError::ByteLimit => Self::new(StatusCode::TOO_MANY_REQUESTS, "byte_limit"),
            AnonymousError::BodyTooLarge => {
                Self::new(StatusCode::PAYLOAD_TOO_LARGE, "body_too_large")
            }
            AnonymousError::InvalidClaim => Self::new(StatusCode::GONE, "invalid_claim"),
            AnonymousError::Storage(_) => Self::internal(),
        }
    }
}

impl IntoResponse for AnonymousApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({
                "error": {
                    "code": self.code
                }
            })),
        )
            .into_response()
    }
}

fn summary(
    exposure_id: Uuid,
    workspace_id: Option<Uuid>,
    created_at: u64,
    expires_at: Option<u64>,
    request_count: u32,
    retained_bytes: u64,
) -> AnonymousExposureSummary {
    AnonymousExposureSummary {
        exposure_id,
        workspace_id,
        created_at_unix_seconds: created_at,
        expires_at_unix_seconds: expires_at,
        request_count,
        retained_bytes,
        request_limit: ANONYMOUS_REQUEST_LIMIT,
        max_body_bytes: ANONYMOUS_MAX_BODY_BYTES,
        max_retained_bytes: ANONYMOUS_MAX_RETAINED_BYTES,
        claimed: workspace_id.is_some(),
    }
}

fn summary_from_raw(
    raw: (String, Option<String>, i64, i64, i64, i64),
    now: i64,
) -> Result<AnonymousExposureSummary, AnonymousError> {
    let exposure_id: Uuid = raw
        .0
        .parse()
        .map_err(|error| AnonymousError::Storage(format!("invalid exposure id: {error}")))?;
    let workspace_id = raw
        .1
        .map(|value| {
            value
                .parse()
                .map_err(|error| AnonymousError::Storage(format!("invalid workspace id: {error}")))
        })
        .transpose()?;
    if workspace_id.is_none() && now >= raw.3 {
        return Err(AnonymousError::Expired);
    }
    let created_at = u64_from_i64(raw.2)?;
    let expires_at = if workspace_id.is_some() {
        None
    } else {
        Some(u64_from_i64(raw.3)?)
    };
    let request_count =
        u32::try_from(raw.4).map_err(|error| AnonymousError::Storage(error.to_string()))?;
    let retained_bytes = u64_from_i64(raw.5)?;
    Ok(summary(
        exposure_id,
        workspace_id,
        created_at,
        expires_at,
        request_count,
        retained_bytes,
    ))
}

fn interaction_from_raw(
    exposure_id: Uuid,
    raw: (
        String,
        i64,
        i64,
        String,
        String,
        Option<String>,
        String,
        Vec<u8>,
    ),
) -> Result<StoredInteraction, AnonymousError> {
    let interaction_id = raw
        .0
        .parse()
        .map_err(|error| AnonymousError::Storage(format!("invalid interaction id: {error}")))?;
    let sequence =
        u32::try_from(raw.1).map_err(|error| AnonymousError::Storage(error.to_string()))?;
    let received_at_unix_ms = u64_from_i64(raw.2)?;
    let headers =
        serde_json::from_str(&raw.6).map_err(|error| AnonymousError::Storage(error.to_string()))?;
    Ok(StoredInteraction {
        interaction_id,
        exposure_id,
        sequence,
        received_at_unix_ms,
        method: raw.3,
        path: raw.4,
        query: raw.5,
        headers,
        body: raw.7,
    })
}

fn check_capture_policy(
    claimed: bool,
    expires_at: i64,
    request_count: i64,
    retained_bytes: i64,
    now: i64,
    body_len: u64,
) -> Result<(), AnonymousError> {
    if !claimed && now >= expires_at {
        return Err(AnonymousError::Expired);
    }
    if request_count >= i64::from(ANONYMOUS_REQUEST_LIMIT) {
        return Err(AnonymousError::RequestLimit);
    }
    let retained_bytes = u64_from_i64(retained_bytes)?;
    if retained_bytes.saturating_add(body_len) > ANONYMOUS_MAX_RETAINED_BYTES {
        return Err(AnonymousError::ByteLimit);
    }
    Ok(())
}

fn purge_sqlite(tx: &rusqlite::Transaction<'_>, now: i64) -> Result<(), AnonymousError> {
    tx.execute(
        "DELETE FROM anonymous_interactions
         WHERE exposure_id IN (
             SELECT exposure_id FROM anonymous_exposures
             WHERE workspace_id IS NULL AND expires_at <= ?1
         )",
        [now],
    )
    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
    tx.execute(
        "DELETE FROM anonymous_exposures
         WHERE workspace_id IS NULL AND expires_at <= ?1",
        [now],
    )
    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
    Ok(())
}

fn purge_postgres(tx: &mut postgres::Transaction<'_>, now: i64) -> Result<(), AnonymousError> {
    tx.execute(
        "DELETE FROM anonymous_interactions
         WHERE exposure_id IN (
             SELECT exposure_id FROM anonymous_exposures
             WHERE workspace_id IS NULL AND expires_at <= $1
         )",
        &[&now],
    )
    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
    tx.execute(
        "DELETE FROM anonymous_exposures
         WHERE workspace_id IS NULL AND expires_at <= $1",
        &[&now],
    )
    .map_err(|error| AnonymousError::Storage(error.to_string()))?;
    Ok(())
}

fn anonymous_principal(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers
        .get(PRINCIPAL_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|value| valid_principal(value))
    {
        return Some(value.to_owned());
    }

    headers
        .get(COOKIE)
        .and_then(|value| value.to_str().ok())
        .and_then(|cookies| {
            cookies.split(';').find_map(|cookie| {
                let (name, value) = cookie.trim().split_once('=')?;
                (name == PRINCIPAL_COOKIE && valid_principal(value)).then(|| value.to_owned())
            })
        })
}

fn valid_principal(value: &str) -> bool {
    value.starts_with("hooktry_ap_") && value.len() == "hooktry_ap_".len() + 64
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
}

fn capture_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.to_string(), value.to_owned()))
        })
        .collect()
}

fn advisory_lock_key(digest: &[u8; 32]) -> i64 {
    let bytes: [u8; 8] = digest[..8]
        .try_into()
        .expect("SHA-256 digest prefix is always 8 bytes");
    i64::from_be_bytes(bytes)
}

fn token_digest(token: &str) -> [u8; 32] {
    digest(&SHA256, token.as_bytes())
        .as_ref()
        .try_into()
        .expect("SHA-256 digest is always 32 bytes")
}

fn random_principal() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    format!("hooktry_ap_{}", hex_encode(&bytes))
}

fn random_capability(prefix: &str) -> String {
    let mut bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut bytes);
    format!("{prefix}{}", hex_encode(&bytes))
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("write to String cannot fail");
    }
    encoded
}

fn websocket_base_url(public_base_url: &str) -> String {
    if let Some(rest) = public_base_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = public_base_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        public_base_url.to_owned()
    }
}

fn unix_seconds_now() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp()).expect("current Unix timestamp is non-negative")
}

fn unix_millis_now() -> u64 {
    u64::try_from(chrono::Utc::now().timestamp_millis())
        .expect("current Unix timestamp is non-negative")
}

fn i64_from_u64(value: u64) -> Result<i64, AnonymousError> {
    i64::try_from(value).map_err(|error| AnonymousError::Storage(error.to_string()))
}

fn u64_from_i64(value: i64) -> Result<u64, AnonymousError> {
    u64::try_from(value).map_err(|error| AnonymousError::Storage(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn principal_is_limited_to_three_active_exposures() {
        let service = AnonymousExposureService::new(
            AnonymousExposureStore::default(),
            "https://hooktry.test",
        );
        let principal = random_principal();
        for _ in 0..ANONYMOUS_ACTIVE_LIMIT {
            service
                .provision(Some(principal.clone()))
                .await
                .expect("provision within limit");
        }
        assert_eq!(
            service.provision(Some(principal)).await.unwrap_err(),
            AnonymousError::ActiveLimit
        );
    }

    #[tokio::test]
    async fn body_budget_and_request_budget_are_enforced() {
        let service = AnonymousExposureService::new(
            AnonymousExposureStore::default(),
            "https://hooktry.test",
        );
        let provision = service.provision(None).await.unwrap();
        let hook_token = provision
            .hook_url
            .rsplit('/')
            .next()
            .expect("hook token")
            .to_owned();

        for _ in 0..ANONYMOUS_REQUEST_LIMIT {
            service
                .capture(
                    &hook_token,
                    "POST".to_owned(),
                    "/".to_owned(),
                    None,
                    Vec::new(),
                    Vec::new(),
                )
                .await
                .unwrap();
        }
        assert_eq!(
            service
                .capture(
                    &hook_token,
                    "POST".to_owned(),
                    "/".to_owned(),
                    None,
                    Vec::new(),
                    Vec::new(),
                )
                .await
                .unwrap_err(),
            AnonymousError::RequestLimit
        );
    }

    #[derive(Default)]
    struct RecordingInteractionStream {
        published: Mutex<Vec<StoredInteraction>>,
    }

    impl AnonymousInteractionStream for RecordingInteractionStream {
        fn publish(&self, interaction: StoredInteraction) {
            self.published
                .lock()
                .expect("recording stream poisoned")
                .push(interaction);
        }

        fn subscribe(
            &self,
            _exposure_id: Uuid,
        ) -> Box<dyn ports::AnonymousInteractionSubscription> {
            Box::new(ClosedSubscription)
        }
    }

    struct ClosedSubscription;

    impl ports::AnonymousInteractionSubscription for ClosedSubscription {
        fn recv(
            &mut self,
        ) -> ports::PortFuture<'_, Result<StoredInteraction, InteractionStreamError>> {
            Box::pin(async { Err(InteractionStreamError::Closed) })
        }
    }

    #[tokio::test]
    async fn application_service_uses_injected_realtime_port() {
        let stream = Arc::new(RecordingInteractionStream::default());
        let service = AnonymousExposureService::with_ports(
            Arc::new(AnonymousExposureStore::default()),
            stream.clone(),
            "https://hooktry.test",
        );
        let provision = service.provision(None).await.unwrap();
        let hook_token = provision.hook_url.rsplit('/').next().unwrap();

        service
            .capture(
                hook_token,
                "POST".to_owned(),
                "/portable".to_owned(),
                None,
                Vec::new(),
                b"portable".to_vec(),
            )
            .await
            .unwrap();

        let published = stream.published.lock().expect("recording stream poisoned");
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].path, "/portable");
        assert_eq!(published[0].body, b"portable");
    }

    #[test]
    fn capability_tokens_use_typed_lowercase_hex_format() {
        for prefix in ["hk_", "vw_", "cl_"] {
            let token = random_capability(prefix);
            assert!(token.starts_with(prefix));
            let suffix = &token[prefix.len()..];
            assert_eq!(suffix.len(), 32);
            assert!(
                suffix
                    .chars()
                    .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
            );
        }
    }

    #[test]
    fn websocket_url_tracks_public_scheme() {
        assert_eq!(
            websocket_base_url("https://hooktry.test"),
            "wss://hooktry.test"
        );
        assert_eq!(
            websocket_base_url("http://127.0.0.1:8080"),
            "ws://127.0.0.1:8080"
        );
    }
}
