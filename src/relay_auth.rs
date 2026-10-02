use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use postgres::{Client, NoTls};
use rand::RngCore;
use ring::digest::{SHA256, digest};
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCapability {
    pub token: String,
    pub exposure_id: Uuid,
    pub expires_at: SystemTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityError {
    Invalid,
    Expired,
    Revoked,
    WrongExposure,
    Storage(String),
}

#[derive(Debug, Clone)]
struct CapabilityRecord {
    exposure_id: Uuid,
    expires_at: SystemTime,
    revoked: bool,
}

#[derive(Clone)]
enum CapabilityBackend {
    Memory(Arc<RwLock<HashMap<[u8; 32], CapabilityRecord>>>),
    Sqlite(Arc<Mutex<Connection>>),
    Postgres(Arc<Mutex<Client>>),
}

#[derive(Clone)]
pub struct CapabilityStore {
    backend: CapabilityBackend,
}

impl Default for CapabilityStore {
    fn default() -> Self {
        Self {
            backend: CapabilityBackend::Memory(Arc::new(RwLock::new(HashMap::new()))),
        }
    }
}

impl CapabilityStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, CapabilityError> {
        let connection =
            Connection::open(path).map_err(|error| CapabilityError::Storage(error.to_string()))?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS runtime_capabilities (
                    token_digest BLOB PRIMARY KEY,
                    exposure_id TEXT NOT NULL,
                    expires_at INTEGER NOT NULL,
                    revoked INTEGER NOT NULL DEFAULT 0
                );
                CREATE INDEX IF NOT EXISTS runtime_capabilities_exposure
                    ON runtime_capabilities(exposure_id);",
            )
            .map_err(|error| CapabilityError::Storage(error.to_string()))?;

        Ok(Self {
            backend: CapabilityBackend::Sqlite(Arc::new(Mutex::new(connection))),
        })
    }

    pub fn open_postgres(database_url: &str) -> Result<Self, CapabilityError> {
        let mut client = Client::connect(database_url, NoTls)
            .map_err(|error| CapabilityError::Storage(error.to_string()))?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS runtime_capabilities (
                    token_digest BYTEA PRIMARY KEY,
                    exposure_id TEXT NOT NULL,
                    expires_at BIGINT NOT NULL,
                    revoked BOOLEAN NOT NULL DEFAULT FALSE
                );
                CREATE INDEX IF NOT EXISTS runtime_capabilities_exposure
                    ON runtime_capabilities(exposure_id);",
            )
            .map_err(|error| CapabilityError::Storage(error.to_string()))?;

        Ok(Self {
            backend: CapabilityBackend::Postgres(Arc::new(Mutex::new(client))),
        })
    }

    pub async fn issue_async(
        &self,
        exposure_id: Uuid,
        ttl: Duration,
    ) -> Result<RuntimeCapability, CapabilityError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.issue(exposure_id, ttl))
            .await
            .map_err(|error| CapabilityError::Storage(error.to_string()))?
    }

    pub async fn rotate_exposure_async(
        &self,
        exposure_id: Uuid,
        ttl: Duration,
    ) -> Result<RuntimeCapability, CapabilityError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.rotate_exposure(exposure_id, ttl))
            .await
            .map_err(|error| CapabilityError::Storage(error.to_string()))?
    }

    pub async fn authorize_async(
        &self,
        exposure_id: Uuid,
        token: &str,
    ) -> Result<(), CapabilityError> {
        let store = self.clone();
        let token = token.to_owned();
        tokio::task::spawn_blocking(move || store.authorize(exposure_id, &token))
            .await
            .map_err(|error| CapabilityError::Storage(error.to_string()))?
    }

    pub async fn revoke_exposure_async(&self, exposure_id: Uuid) -> Result<(), CapabilityError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.revoke_exposure(exposure_id))
            .await
            .map_err(|error| CapabilityError::Storage(error.to_string()))?
    }

    pub async fn revoke_async(&self, token: &str) -> Result<(), CapabilityError> {
        let store = self.clone();
        let token = token.to_owned();
        tokio::task::spawn_blocking(move || store.revoke(&token))
            .await
            .map_err(|error| CapabilityError::Storage(error.to_string()))?
    }

    pub fn issue(
        &self,
        exposure_id: Uuid,
        ttl: Duration,
    ) -> Result<RuntimeCapability, CapabilityError> {
        let token = random_token();
        let expires_at = SystemTime::now() + ttl;
        let record = CapabilityRecord {
            exposure_id,
            expires_at,
            revoked: false,
        };
        self.insert(token_digest(&token), &record)?;

        Ok(RuntimeCapability {
            token,
            exposure_id,
            expires_at,
        })
    }

    pub fn rotate_exposure(
        &self,
        exposure_id: Uuid,
        ttl: Duration,
    ) -> Result<RuntimeCapability, CapabilityError> {
        let token = random_token();
        let expires_at = SystemTime::now() + ttl;
        let digest = token_digest(&token);
        let expires_at_unix = unix_seconds(expires_at)?;

        match &self.backend {
            CapabilityBackend::Memory(inner) => {
                let mut store = inner.write().expect("capability store poisoned");
                for record in store
                    .values_mut()
                    .filter(|record| record.exposure_id == exposure_id)
                {
                    record.revoked = true;
                }
                store.insert(
                    digest,
                    CapabilityRecord {
                        exposure_id,
                        expires_at,
                        revoked: false,
                    },
                );
            }
            CapabilityBackend::Sqlite(connection) => {
                let mut connection = connection.lock().expect("capability store poisoned");
                let transaction = connection
                    .transaction()
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
                transaction
                    .execute(
                        "UPDATE runtime_capabilities SET revoked = 1 WHERE exposure_id = ?1",
                        [exposure_id.to_string()],
                    )
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
                transaction
                    .execute(
                        "INSERT INTO runtime_capabilities
                            (token_digest, exposure_id, expires_at, revoked)
                         VALUES (?1, ?2, ?3, 0)",
                        params![digest.as_slice(), exposure_id.to_string(), expires_at_unix],
                    )
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
                transaction
                    .commit()
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
            }
            CapabilityBackend::Postgres(client) => {
                let mut client = client.lock().expect("capability store poisoned");
                let mut transaction = client
                    .transaction()
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
                let exposure_id_text = exposure_id.to_string();
                transaction
                    .execute(
                        "UPDATE runtime_capabilities SET revoked = TRUE WHERE exposure_id = $1",
                        &[&exposure_id_text],
                    )
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
                transaction
                    .execute(
                        "INSERT INTO runtime_capabilities
                            (token_digest, exposure_id, expires_at, revoked)
                         VALUES ($1, $2, $3, FALSE)",
                        &[&digest.as_slice(), &exposure_id_text, &expires_at_unix],
                    )
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
                transaction
                    .commit()
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
            }
        }

        Ok(RuntimeCapability {
            token,
            exposure_id,
            expires_at,
        })
    }

    pub fn authorize(&self, exposure_id: Uuid, token: &str) -> Result<(), CapabilityError> {
        let record = self
            .find(token_digest(token))?
            .ok_or(CapabilityError::Invalid)?;
        if record.revoked {
            return Err(CapabilityError::Revoked);
        }
        if record.exposure_id != exposure_id {
            return Err(CapabilityError::WrongExposure);
        }
        if SystemTime::now() >= record.expires_at {
            return Err(CapabilityError::Expired);
        }
        Ok(())
    }

    pub fn revoke(&self, token: &str) -> Result<(), CapabilityError> {
        let digest = token_digest(token);
        match &self.backend {
            CapabilityBackend::Memory(inner) => {
                let mut store = inner.write().expect("capability store poisoned");
                let record = store.get_mut(&digest).ok_or(CapabilityError::Invalid)?;
                record.revoked = true;
                Ok(())
            }
            CapabilityBackend::Sqlite(connection) => {
                let updated = connection
                    .lock()
                    .expect("capability store poisoned")
                    .execute(
                        "UPDATE runtime_capabilities SET revoked = 1 WHERE token_digest = ?1",
                        params![digest.as_slice()],
                    )
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
                if updated == 0 {
                    Err(CapabilityError::Invalid)
                } else {
                    Ok(())
                }
            }
            CapabilityBackend::Postgres(client) => {
                let updated = client
                    .lock()
                    .expect("capability store poisoned")
                    .execute(
                        "UPDATE runtime_capabilities SET revoked = TRUE WHERE token_digest = $1",
                        &[&digest.as_slice()],
                    )
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
                if updated == 0 {
                    Err(CapabilityError::Invalid)
                } else {
                    Ok(())
                }
            }
        }
    }

    pub fn revoke_exposure(&self, exposure_id: Uuid) -> Result<(), CapabilityError> {
        match &self.backend {
            CapabilityBackend::Memory(inner) => {
                for record in inner
                    .write()
                    .expect("capability store poisoned")
                    .values_mut()
                    .filter(|record| record.exposure_id == exposure_id)
                {
                    record.revoked = true;
                }
                Ok(())
            }
            CapabilityBackend::Sqlite(connection) => {
                connection
                    .lock()
                    .expect("capability store poisoned")
                    .execute(
                        "UPDATE runtime_capabilities SET revoked = 1 WHERE exposure_id = ?1",
                        [exposure_id.to_string()],
                    )
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
                Ok(())
            }
            CapabilityBackend::Postgres(client) => {
                let exposure_id = exposure_id.to_string();
                client
                    .lock()
                    .expect("capability store poisoned")
                    .execute(
                        "UPDATE runtime_capabilities SET revoked = TRUE WHERE exposure_id = $1",
                        &[&exposure_id],
                    )
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
                Ok(())
            }
        }
    }

    fn insert(&self, digest: [u8; 32], record: &CapabilityRecord) -> Result<(), CapabilityError> {
        match &self.backend {
            CapabilityBackend::Memory(inner) => {
                inner
                    .write()
                    .expect("capability store poisoned")
                    .insert(digest, record.clone());
                Ok(())
            }
            CapabilityBackend::Sqlite(connection) => {
                connection
                    .lock()
                    .expect("capability store poisoned")
                    .execute(
                        "INSERT INTO runtime_capabilities
                            (token_digest, exposure_id, expires_at, revoked)
                         VALUES (?1, ?2, ?3, ?4)",
                        params![
                            digest.as_slice(),
                            record.exposure_id.to_string(),
                            unix_seconds(record.expires_at)?,
                            record.revoked
                        ],
                    )
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
                Ok(())
            }
            CapabilityBackend::Postgres(client) => {
                let exposure_id = record.exposure_id.to_string();
                let expires_at = unix_seconds(record.expires_at)?;
                client
                    .lock()
                    .expect("capability store poisoned")
                    .execute(
                        "INSERT INTO runtime_capabilities
                            (token_digest, exposure_id, expires_at, revoked)
                         VALUES ($1, $2, $3, $4)",
                        &[
                            &digest.as_slice(),
                            &exposure_id,
                            &expires_at,
                            &record.revoked,
                        ],
                    )
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
                Ok(())
            }
        }
    }

    fn find(&self, digest: [u8; 32]) -> Result<Option<CapabilityRecord>, CapabilityError> {
        match &self.backend {
            CapabilityBackend::Memory(inner) => Ok(inner
                .read()
                .expect("capability store poisoned")
                .get(&digest)
                .cloned()),
            CapabilityBackend::Sqlite(connection) => connection
                .lock()
                .expect("capability store poisoned")
                .query_row(
                    "SELECT exposure_id, expires_at, revoked
                     FROM runtime_capabilities
                     WHERE token_digest = ?1",
                    params![digest.as_slice()],
                    |row| {
                        let exposure_id: String = row.get(0)?;
                        let expires_at: i64 = row.get(1)?;
                        let revoked: bool = row.get(2)?;
                        Ok((exposure_id, expires_at, revoked))
                    },
                )
                .optional()
                .map_err(|error| CapabilityError::Storage(error.to_string()))?
                .map(capability_record_from_raw)
                .transpose(),
            CapabilityBackend::Postgres(client) => {
                let row = client
                    .lock()
                    .expect("capability store poisoned")
                    .query_opt(
                        "SELECT exposure_id, expires_at, revoked
                         FROM runtime_capabilities
                         WHERE token_digest = $1",
                        &[&digest.as_slice()],
                    )
                    .map_err(|error| CapabilityError::Storage(error.to_string()))?;
                row.map(|row| {
                    capability_record_from_raw((
                        row.get::<_, String>(0),
                        row.get::<_, i64>(1),
                        row.get::<_, bool>(2),
                    ))
                })
                .transpose()
            }
        }
    }
}

fn capability_record_from_raw(
    (exposure_id, expires_at, revoked): (String, i64, bool),
) -> Result<CapabilityRecord, CapabilityError> {
    let exposure_id = exposure_id
        .parse()
        .map_err(|error| CapabilityError::Storage(format!("invalid exposure id: {error}")))?;
    let expires_at = UNIX_EPOCH
        + Duration::from_secs(u64::try_from(expires_at).map_err(|error| {
            CapabilityError::Storage(format!("invalid capability expiry: {error}"))
        })?);
    Ok(CapabilityRecord {
        exposure_id,
        expires_at,
        revoked,
    })
}

fn unix_seconds(time: SystemTime) -> Result<i64, CapabilityError> {
    let seconds = time
        .duration_since(UNIX_EPOCH)
        .map_err(|error| CapabilityError::Storage(error.to_string()))?
        .as_secs();
    i64::try_from(seconds).map_err(|error| CapabilityError::Storage(error.to_string()))
}

pub(crate) fn token_digest(token: &str) -> [u8; 32] {
    let digest = digest(&SHA256, token.as_bytes());
    digest
        .as_ref()
        .try_into()
        .expect("SHA-256 digest is always 32 bytes")
}

fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut encoded, "{byte:02x}").expect("write to String cannot fail");
    }

    format!("hooktry_rt_{encoded}")
}
