use std::{
    path::Path,
    sync::{Arc, Mutex},
};

use postgres::{Client, NoTls};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostedExposureRecord {
    pub exposure_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<Uuid>,
    pub name: String,
    pub target_port: u16,
    pub public_url: String,
    pub runtime_url: String,
    pub capability_expires_at_unix_seconds: u64,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostedStateError {
    Storage(String),
    InvalidRecord(String),
    NotFound,
}

#[derive(Clone)]
enum HostedExposureBackend {
    Sqlite(Arc<Mutex<Connection>>),
    Postgres(Arc<Mutex<Client>>),
}

#[derive(Clone)]
pub struct HostedExposureStore {
    backend: HostedExposureBackend,
}

impl Default for HostedExposureStore {
    fn default() -> Self {
        Self::in_memory().expect("create in-memory hosted exposure store")
    }
}

impl HostedExposureStore {
    pub fn in_memory() -> Result<Self, HostedStateError> {
        let connection = Connection::open_in_memory()
            .map_err(|error| HostedStateError::Storage(error.to_string()))?;
        Self::from_sqlite_connection(connection)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self, HostedStateError> {
        let connection =
            Connection::open(path).map_err(|error| HostedStateError::Storage(error.to_string()))?;
        Self::from_sqlite_connection(connection)
    }

    pub fn open_postgres(database_url: &str) -> Result<Self, HostedStateError> {
        let mut client = Client::connect(database_url, NoTls)
            .map_err(|error| HostedStateError::Storage(error.to_string()))?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS hosted_exposures (
                    exposure_id TEXT PRIMARY KEY,
                    workspace_id TEXT,
                    name TEXT NOT NULL,
                    target_port INTEGER NOT NULL,
                    public_url TEXT NOT NULL,
                    runtime_url TEXT NOT NULL,
                    capability_expires_at BIGINT NOT NULL,
                    revoked BOOLEAN NOT NULL DEFAULT FALSE
                );
                ALTER TABLE hosted_exposures ADD COLUMN IF NOT EXISTS workspace_id TEXT;
                CREATE INDEX IF NOT EXISTS hosted_exposures_workspace
                    ON hosted_exposures(workspace_id);",
            )
            .map_err(|error| HostedStateError::Storage(error.to_string()))?;

        Ok(Self {
            backend: HostedExposureBackend::Postgres(Arc::new(Mutex::new(client))),
        })
    }

    fn from_sqlite_connection(connection: Connection) -> Result<Self, HostedStateError> {
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS hosted_exposures (
                    exposure_id TEXT PRIMARY KEY,
                    workspace_id TEXT,
                    name TEXT NOT NULL,
                    target_port INTEGER NOT NULL,
                    public_url TEXT NOT NULL,
                    runtime_url TEXT NOT NULL,
                    capability_expires_at INTEGER NOT NULL,
                    revoked INTEGER NOT NULL DEFAULT 0
                );",
            )
            .map_err(|error| HostedStateError::Storage(error.to_string()))?;

        let has_workspace_id = {
            let mut statement = connection
                .prepare("PRAGMA table_info(hosted_exposures)")
                .map_err(|error| HostedStateError::Storage(error.to_string()))?;
            let mut rows = statement
                .query([])
                .map_err(|error| HostedStateError::Storage(error.to_string()))?;
            let mut found = false;
            while let Some(row) = rows
                .next()
                .map_err(|error| HostedStateError::Storage(error.to_string()))?
            {
                let name: String = row
                    .get(1)
                    .map_err(|error| HostedStateError::Storage(error.to_string()))?;
                if name == "workspace_id" {
                    found = true;
                    break;
                }
            }
            found
        };
        if !has_workspace_id {
            connection
                .execute(
                    "ALTER TABLE hosted_exposures ADD COLUMN workspace_id TEXT",
                    [],
                )
                .map_err(|error| HostedStateError::Storage(error.to_string()))?;
        }
        connection
            .execute(
                "CREATE INDEX IF NOT EXISTS hosted_exposures_workspace
                 ON hosted_exposures(workspace_id)",
                [],
            )
            .map_err(|error| HostedStateError::Storage(error.to_string()))?;

        Ok(Self {
            backend: HostedExposureBackend::Sqlite(Arc::new(Mutex::new(connection))),
        })
    }

    pub async fn save_async(&self, record: HostedExposureRecord) -> Result<(), HostedStateError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.save(&record))
            .await
            .map_err(|error| HostedStateError::Storage(error.to_string()))?
    }

    pub async fn get_async(
        &self,
        exposure_id: Uuid,
    ) -> Result<Option<HostedExposureRecord>, HostedStateError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.get(exposure_id))
            .await
            .map_err(|error| HostedStateError::Storage(error.to_string()))?
    }

    pub async fn update_capability_expiry_async(
        &self,
        exposure_id: Uuid,
        expires_at_unix_seconds: u64,
    ) -> Result<(), HostedStateError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.update_capability_expiry(exposure_id, expires_at_unix_seconds)
        })
        .await
        .map_err(|error| HostedStateError::Storage(error.to_string()))?
    }

    pub async fn revoke_async(&self, exposure_id: Uuid) -> Result<(), HostedStateError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.revoke(exposure_id))
            .await
            .map_err(|error| HostedStateError::Storage(error.to_string()))?
    }

    pub fn save(&self, record: &HostedExposureRecord) -> Result<(), HostedStateError> {
        let target_port = i32::from(record.target_port);
        let expires_at = i64::try_from(record.capability_expires_at_unix_seconds)
            .map_err(|error| HostedStateError::InvalidRecord(error.to_string()))?;

        match &self.backend {
            HostedExposureBackend::Sqlite(connection) => {
                connection
                    .lock()
                    .expect("hosted exposure store poisoned")
                    .execute(
                        "INSERT INTO hosted_exposures
                            (exposure_id, workspace_id, name, target_port, public_url, runtime_url,
                             capability_expires_at, revoked)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                        params![
                            record.exposure_id.to_string(),
                            record.workspace_id.map(|id| id.to_string()),
                            &record.name,
                            i64::from(record.target_port),
                            &record.public_url,
                            &record.runtime_url,
                            expires_at,
                            record.revoked
                        ],
                    )
                    .map_err(|error| HostedStateError::Storage(error.to_string()))?;
                Ok(())
            }
            HostedExposureBackend::Postgres(client) => {
                let exposure_id = record.exposure_id.to_string();
                client
                    .lock()
                    .expect("hosted exposure store poisoned")
                    .execute(
                        "INSERT INTO hosted_exposures
                            (exposure_id, workspace_id, name, target_port, public_url, runtime_url,
                             capability_expires_at, revoked)
                         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                        &[
                            &exposure_id,
                            &record.workspace_id.map(|id| id.to_string()),
                            &record.name,
                            &target_port,
                            &record.public_url,
                            &record.runtime_url,
                            &expires_at,
                            &record.revoked,
                        ],
                    )
                    .map_err(|error| HostedStateError::Storage(error.to_string()))?;
                Ok(())
            }
        }
    }

    pub fn update_capability_expiry(
        &self,
        exposure_id: Uuid,
        expires_at_unix_seconds: u64,
    ) -> Result<(), HostedStateError> {
        let expires_at = i64::try_from(expires_at_unix_seconds)
            .map_err(|error| HostedStateError::InvalidRecord(error.to_string()))?;

        let updated = match &self.backend {
            HostedExposureBackend::Sqlite(connection) => connection
                .lock()
                .expect("hosted exposure store poisoned")
                .execute(
                    "UPDATE hosted_exposures
                     SET capability_expires_at = ?1
                     WHERE exposure_id = ?2 AND revoked = 0",
                    params![expires_at, exposure_id.to_string()],
                )
                .map_err(|error| HostedStateError::Storage(error.to_string()))?,
            HostedExposureBackend::Postgres(client) => {
                let exposure_id = exposure_id.to_string();
                client
                    .lock()
                    .expect("hosted exposure store poisoned")
                    .execute(
                        "UPDATE hosted_exposures
                         SET capability_expires_at = $1
                         WHERE exposure_id = $2 AND revoked = FALSE",
                        &[&expires_at, &exposure_id],
                    )
                    .map_err(|error| HostedStateError::Storage(error.to_string()))?
                    as usize
            }
        };

        if updated == 0 {
            Err(HostedStateError::NotFound)
        } else {
            Ok(())
        }
    }

    pub fn get(&self, exposure_id: Uuid) -> Result<Option<HostedExposureRecord>, HostedStateError> {
        match &self.backend {
            HostedExposureBackend::Sqlite(connection) => {
                let raw = connection
                    .lock()
                    .expect("hosted exposure store poisoned")
                    .query_row(
                        "SELECT workspace_id, name, target_port, public_url, runtime_url,
                                capability_expires_at, revoked
                         FROM hosted_exposures
                         WHERE exposure_id = ?1",
                        [exposure_id.to_string()],
                        |row| {
                            let workspace_id: Option<String> = row.get(0)?;
                            let name: String = row.get(1)?;
                            let target_port: i64 = row.get(2)?;
                            let public_url: String = row.get(3)?;
                            let runtime_url: String = row.get(4)?;
                            let capability_expires_at: i64 = row.get(5)?;
                            let revoked: bool = row.get(6)?;
                            Ok((
                                workspace_id,
                                name,
                                target_port,
                                public_url,
                                runtime_url,
                                capability_expires_at,
                                revoked,
                            ))
                        },
                    )
                    .optional()
                    .map_err(|error| HostedStateError::Storage(error.to_string()))?;

                raw.map(|raw| hosted_record_from_raw(exposure_id, raw))
                    .transpose()
            }
            HostedExposureBackend::Postgres(client) => {
                let exposure_id_text = exposure_id.to_string();
                let row = client
                    .lock()
                    .expect("hosted exposure store poisoned")
                    .query_opt(
                        "SELECT workspace_id, name, target_port, public_url, runtime_url,
                                capability_expires_at, revoked
                         FROM hosted_exposures
                         WHERE exposure_id = $1",
                        &[&exposure_id_text],
                    )
                    .map_err(|error| HostedStateError::Storage(error.to_string()))?;

                row.map(|row| {
                    hosted_record_from_raw(
                        exposure_id,
                        (
                            row.get::<_, Option<String>>(0),
                            row.get::<_, String>(1),
                            i64::from(row.get::<_, i32>(2)),
                            row.get::<_, String>(3),
                            row.get::<_, String>(4),
                            row.get::<_, i64>(5),
                            row.get::<_, bool>(6),
                        ),
                    )
                })
                .transpose()
            }
        }
    }

    pub async fn list_for_workspace_async(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<HostedExposureRecord>, HostedStateError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.list_for_workspace(workspace_id))
            .await
            .map_err(|error| HostedStateError::Storage(error.to_string()))?
    }

    pub fn list_for_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<HostedExposureRecord>, HostedStateError> {
        let workspace_id_text = workspace_id.to_string();
        match &self.backend {
            HostedExposureBackend::Sqlite(connection) => {
                let connection = connection.lock().expect("hosted exposure store poisoned");
                let mut statement = connection
                    .prepare(
                        "SELECT exposure_id, workspace_id, name, target_port, public_url, runtime_url,
                                capability_expires_at, revoked
                         FROM hosted_exposures
                         WHERE workspace_id = ?1
                         ORDER BY exposure_id",
                    )
                    .map_err(|error| HostedStateError::Storage(error.to_string()))?;
                let rows = statement
                    .query_map([&workspace_id_text], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, i64>(6)?,
                            row.get::<_, bool>(7)?,
                        ))
                    })
                    .map_err(|error| HostedStateError::Storage(error.to_string()))?;
                rows.map(|row| {
                    let (
                        exposure_id,
                        workspace_id,
                        name,
                        target_port,
                        public_url,
                        runtime_url,
                        expires_at,
                        revoked,
                    ) = row.map_err(|error| HostedStateError::Storage(error.to_string()))?;
                    let exposure_id = exposure_id.parse().map_err(|error| {
                        HostedStateError::InvalidRecord(format!("invalid exposure id: {error}"))
                    })?;
                    hosted_record_from_raw(
                        exposure_id,
                        (
                            workspace_id,
                            name,
                            target_port,
                            public_url,
                            runtime_url,
                            expires_at,
                            revoked,
                        ),
                    )
                })
                .collect()
            }
            HostedExposureBackend::Postgres(client) => {
                let rows = client
                    .lock()
                    .expect("hosted exposure store poisoned")
                    .query(
                        "SELECT exposure_id, workspace_id, name, target_port, public_url, runtime_url,
                                capability_expires_at, revoked
                         FROM hosted_exposures
                         WHERE workspace_id = $1
                         ORDER BY exposure_id",
                        &[&workspace_id_text],
                    )
                    .map_err(|error| HostedStateError::Storage(error.to_string()))?;
                rows.into_iter()
                    .map(|row| {
                        let exposure_id: String = row.get(0);
                        let exposure_id = exposure_id.parse().map_err(|error| {
                            HostedStateError::InvalidRecord(format!("invalid exposure id: {error}"))
                        })?;
                        hosted_record_from_raw(
                            exposure_id,
                            (
                                row.get::<_, Option<String>>(1),
                                row.get::<_, String>(2),
                                i64::from(row.get::<_, i32>(3)),
                                row.get::<_, String>(4),
                                row.get::<_, String>(5),
                                row.get::<_, i64>(6),
                                row.get::<_, bool>(7),
                            ),
                        )
                    })
                    .collect()
            }
        }
    }

    pub fn revoke(&self, exposure_id: Uuid) -> Result<(), HostedStateError> {
        let exposure_id = exposure_id.to_string();
        let updated = match &self.backend {
            HostedExposureBackend::Sqlite(connection) => connection
                .lock()
                .expect("hosted exposure store poisoned")
                .execute(
                    "UPDATE hosted_exposures SET revoked = 1 WHERE exposure_id = ?1",
                    [&exposure_id],
                )
                .map_err(|error| HostedStateError::Storage(error.to_string()))?
                as u64,
            HostedExposureBackend::Postgres(client) => client
                .lock()
                .expect("hosted exposure store poisoned")
                .execute(
                    "UPDATE hosted_exposures SET revoked = TRUE WHERE exposure_id = $1",
                    &[&exposure_id],
                )
                .map_err(|error| HostedStateError::Storage(error.to_string()))?,
        };

        if updated == 0 {
            Err(HostedStateError::NotFound)
        } else {
            Ok(())
        }
    }
}

fn hosted_record_from_raw(
    exposure_id: Uuid,
    (workspace_id, name, target_port, public_url, runtime_url, capability_expires_at, revoked): (
        Option<String>,
        String,
        i64,
        String,
        String,
        i64,
        bool,
    ),
) -> Result<HostedExposureRecord, HostedStateError> {
    let workspace_id = workspace_id
        .map(|id| {
            id.parse().map_err(|error| {
                HostedStateError::InvalidRecord(format!("invalid workspace id: {error}"))
            })
        })
        .transpose()?;

    Ok(HostedExposureRecord {
        exposure_id,
        workspace_id,
        name,
        target_port: u16::try_from(target_port)
            .map_err(|error| HostedStateError::InvalidRecord(error.to_string()))?,
        public_url,
        runtime_url,
        capability_expires_at_unix_seconds: u64::try_from(capability_expires_at)
            .map_err(|error| HostedStateError::InvalidRecord(error.to_string()))?,
        revoked,
    })
}
