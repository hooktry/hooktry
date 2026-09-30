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
                    name TEXT NOT NULL,
                    target_port INTEGER NOT NULL,
                    public_url TEXT NOT NULL,
                    runtime_url TEXT NOT NULL,
                    capability_expires_at BIGINT NOT NULL,
                    revoked BOOLEAN NOT NULL DEFAULT FALSE
                );",
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
                    name TEXT NOT NULL,
                    target_port INTEGER NOT NULL,
                    public_url TEXT NOT NULL,
                    runtime_url TEXT NOT NULL,
                    capability_expires_at INTEGER NOT NULL,
                    revoked INTEGER NOT NULL DEFAULT 0
                );",
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
                            (exposure_id, name, target_port, public_url, runtime_url,
                             capability_expires_at, revoked)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                        params![
                            record.exposure_id.to_string(),
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
                            (exposure_id, name, target_port, public_url, runtime_url,
                             capability_expires_at, revoked)
                         VALUES ($1, $2, $3, $4, $5, $6, $7)",
                        &[
                            &exposure_id,
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

    pub fn get(&self, exposure_id: Uuid) -> Result<Option<HostedExposureRecord>, HostedStateError> {
        match &self.backend {
            HostedExposureBackend::Sqlite(connection) => {
                let raw = connection
                    .lock()
                    .expect("hosted exposure store poisoned")
                    .query_row(
                        "SELECT name, target_port, public_url, runtime_url,
                                capability_expires_at, revoked
                         FROM hosted_exposures
                         WHERE exposure_id = ?1",
                        [exposure_id.to_string()],
                        |row| {
                            let name: String = row.get(0)?;
                            let target_port: i64 = row.get(1)?;
                            let public_url: String = row.get(2)?;
                            let runtime_url: String = row.get(3)?;
                            let capability_expires_at: i64 = row.get(4)?;
                            let revoked: bool = row.get(5)?;
                            Ok((
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
                        "SELECT name, target_port, public_url, runtime_url,
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
                            row.get::<_, String>(0),
                            i64::from(row.get::<_, i32>(1)),
                            row.get::<_, String>(2),
                            row.get::<_, String>(3),
                            row.get::<_, i64>(4),
                            row.get::<_, bool>(5),
                        ),
                    )
                })
                .transpose()
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
    (name, target_port, public_url, runtime_url, capability_expires_at, revoked): (
        String,
        i64,
        String,
        String,
        i64,
        bool,
    ),
) -> Result<HostedExposureRecord, HostedStateError> {
    Ok(HostedExposureRecord {
        exposure_id,
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
