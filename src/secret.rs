use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, RwLock},
};

use base64::{engine::general_purpose::STANDARD, Engine};
use postgres::{Client, NoTls};
use rand::RngCore;
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use rusqlite::{Connection, OptionalExtension, params};

use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRef {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretError {
    InvalidName,
    InvalidKey,
    NotFound,
    Crypto,
    Storage(String),
}

#[derive(Clone)]
enum SecretBackend {
    Memory(Arc<RwLock<HashMap<(Uuid, String), StoredSecret>>>),
    Sqlite(Arc<Mutex<Connection>>),
    Postgres(Arc<Mutex<Client>>),
}

#[derive(Clone)]
pub struct SecretStore {
    backend: SecretBackend,
    key: Arc<[u8; 32]>,
}

#[derive(Clone)]
struct StoredSecret {
    reference: SecretRef,
    envelope: String,
    key_version: i32,
}

const KEY_VERSION: i32 = 1;

impl Default for SecretStore {
    fn default() -> Self {
        Self::memory_with_key([0x42; 32])
    }
}

impl SecretStore {
    pub fn memory_with_key(key: [u8; 32]) -> Self {
        Self {
            backend: SecretBackend::Memory(Arc::new(RwLock::new(HashMap::new()))),
            key: Arc::new(key),
        }
    }

    pub fn open(path: impl AsRef<Path>, key: [u8; 32]) -> Result<Self, SecretError> {
        let connection =
            Connection::open(path).map_err(|error| SecretError::Storage(error.to_string()))?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS hosted_secrets (
                    secret_id TEXT PRIMARY KEY,
                    workspace_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    envelope TEXT NOT NULL,
                    key_version INTEGER NOT NULL,
                    UNIQUE(workspace_id, name)
                );
                CREATE INDEX IF NOT EXISTS hosted_secrets_workspace
                    ON hosted_secrets(workspace_id);",
            )
            .map_err(|error| SecretError::Storage(error.to_string()))?;
        Ok(Self {
            backend: SecretBackend::Sqlite(Arc::new(Mutex::new(connection))),
            key: Arc::new(key),
        })
    }

    pub fn open_postgres(database_url: &str, key: [u8; 32]) -> Result<Self, SecretError> {
        let mut client = Client::connect(database_url, NoTls)
            .map_err(|error| SecretError::Storage(error.to_string()))?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS hosted_secrets (
                    secret_id TEXT PRIMARY KEY,
                    workspace_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    envelope TEXT NOT NULL,
                    key_version INTEGER NOT NULL,
                    UNIQUE(workspace_id, name)
                );
                CREATE INDEX IF NOT EXISTS hosted_secrets_workspace
                    ON hosted_secrets(workspace_id);",
            )
            .map_err(|error| SecretError::Storage(error.to_string()))?;
        Ok(Self {
            backend: SecretBackend::Postgres(Arc::new(Mutex::new(client))),
            key: Arc::new(key),
        })
    }

    pub fn put(
        &self,
        workspace_id: Uuid,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<SecretRef, SecretError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(SecretError::InvalidName);
        }
        let reference = SecretRef {
            id: Uuid::now_v7(),
            workspace_id,
            name: name.clone(),
        };
        let envelope = encrypt(
            self.key.as_ref(),
            workspace_id,
            &name,
            value.into().as_bytes(),
        )?;
        self.save(StoredSecret {
            reference: reference.clone(),
            envelope,
            key_version: KEY_VERSION,
        })?;
        Ok(reference)
    }

    pub fn resolve(&self, workspace_id: Uuid, name: &str) -> Result<String, SecretError> {
        let secret = self.find(workspace_id, name)?.ok_or(SecretError::NotFound)?;
        if secret.key_version != KEY_VERSION {
            return Err(SecretError::InvalidKey);
        }
        let bytes = decrypt(
            self.key.as_ref(),
            workspace_id,
            name,
            &secret.envelope,
        )?;
        String::from_utf8(bytes).map_err(|_| SecretError::Crypto)
    }

    pub fn get_ref(&self, workspace_id: Uuid, name: &str) -> Option<SecretRef> {
        self.find(workspace_id, name)
            .ok()
            .flatten()
            .map(|secret| secret.reference)
    }

    fn save(&self, secret: StoredSecret) -> Result<(), SecretError> {
        match &self.backend {
            SecretBackend::Memory(inner) => {
                inner.write().expect("secret store poisoned").insert(
                    (secret.reference.workspace_id, secret.reference.name.clone()),
                    secret,
                );
                Ok(())
            }
            SecretBackend::Sqlite(connection) => {
                connection
                    .lock()
                    .expect("secret store poisoned")
                    .execute(
                    "INSERT INTO hosted_secrets
                        (secret_id, workspace_id, name, envelope, key_version)
                     VALUES (?1, ?2, ?3, ?4, ?5)
                     ON CONFLICT(workspace_id, name) DO UPDATE SET
                        secret_id=excluded.secret_id, envelope=excluded.envelope,
                        key_version=excluded.key_version",
                    params![
                        secret.reference.id.to_string(),
                        secret.reference.workspace_id.to_string(),
                        &secret.reference.name,
                        &secret.envelope,
                        secret.key_version
                    ],
                )
                    .map_err(|error| SecretError::Storage(error.to_string()))?;
                Ok(())
            }
            SecretBackend::Postgres(client) => {
                let id = secret.reference.id.to_string();
                let workspace = secret.reference.workspace_id.to_string();
                client
                    .lock()
                    .expect("secret store poisoned")
                    .execute(
                    "INSERT INTO hosted_secrets
                        (secret_id, workspace_id, name, envelope, key_version)
                     VALUES ($1,$2,$3,$4,$5)
                     ON CONFLICT(workspace_id, name) DO UPDATE SET
                        secret_id=EXCLUDED.secret_id, envelope=EXCLUDED.envelope,
                        key_version=EXCLUDED.key_version",
                    &[
                        &id,
                        &workspace,
                        &secret.reference.name,
                        &secret.envelope,
                        &secret.key_version,
                    ],
                )
                    .map_err(|error| SecretError::Storage(error.to_string()))?;
                Ok(())
            }
        }
    }

    fn find(&self, workspace_id: Uuid, name: &str) -> Result<Option<StoredSecret>, SecretError> {
        match &self.backend {
            SecretBackend::Memory(inner) => Ok(inner
                .read()
                .expect("secret store poisoned")
                .get(&(workspace_id, name.to_owned()))
                .cloned()),
            SecretBackend::Sqlite(connection) => {
                let row = connection
                    .lock()
                    .expect("secret store poisoned")
                    .query_row(
                    "SELECT secret_id,envelope,key_version FROM hosted_secrets
                     WHERE workspace_id=?1 AND name=?2",
                    params![workspace_id.to_string(), name],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i32>(2)?,
                        ))
                    },
                )
                    .optional()
                    .map_err(|error| SecretError::Storage(error.to_string()))?;
                row.map(|(id, envelope, key_version)| {
                    Ok(StoredSecret {
                    reference: SecretRef {
                        id: id.parse().map_err(|error| {
                            SecretError::Storage(format!("invalid secret id: {error}"))
                        })?,
                        workspace_id,
                        name: name.to_owned(),
                    },
                    envelope,
                    key_version,
                    })
                })
                .transpose()
            }
            SecretBackend::Postgres(client) => {
                let workspace = workspace_id.to_string();
                let row = client
                    .lock()
                    .expect("secret store poisoned")
                    .query_opt(
                    "SELECT secret_id,envelope,key_version FROM hosted_secrets
                     WHERE workspace_id=$1 AND name=$2",
                    &[&workspace, &name],
                )
                    .map_err(|error| SecretError::Storage(error.to_string()))?;
                row.map(|row| {
                    Ok(StoredSecret {
                    reference: SecretRef {
                        id: row.get::<_, String>(0).parse().map_err(|error| {
                            SecretError::Storage(format!("invalid secret id: {error}"))
                        })?,
                        workspace_id,
                        name: name.to_owned(),
                    },
                    envelope: row.get(1),
                    key_version: row.get(2),
                    })
                })
                .transpose()
            }
        }
    }
}

pub fn decode_master_key(value: &str) -> Result<[u8; 32], SecretError> {
    let bytes = STANDARD
        .decode(value)
        .map_err(|_| SecretError::InvalidKey)?;
    bytes.try_into().map_err(|_| SecretError::InvalidKey)
}

fn encrypt(
    key: &[u8; 32],
    workspace_id: Uuid,
    name: &str,
    plaintext: &[u8],
) -> Result<String, SecretError> {
    let key =
        LessSafeKey::new(UnboundKey::new(&AES_256_GCM, key).map_err(|_| SecretError::Crypto)?);
    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let aad_text = format!("{workspace_id}:{name}:{KEY_VERSION}");
    let mut in_out = plaintext.to_vec();
    key.seal_in_place_append_tag(nonce, Aad::from(aad_text.as_bytes()), &mut in_out)
        .map_err(|_| SecretError::Crypto)?;
    let mut envelope = nonce_bytes.to_vec();
    envelope.extend_from_slice(&in_out);
    Ok(STANDARD.encode(envelope))
}

fn decrypt(
    key: &[u8; 32],
    workspace_id: Uuid,
    name: &str,
    envelope: &str,
) -> Result<Vec<u8>, SecretError> {
    let envelope = STANDARD.decode(envelope).map_err(|_| SecretError::Crypto)?;
    if envelope.len() < 12 + AES_256_GCM.tag_len() {
        return Err(SecretError::Crypto);
    }
    let (nonce_bytes, ciphertext) = envelope.split_at(12);
    let nonce_bytes: [u8; 12] = nonce_bytes.try_into().map_err(|_| SecretError::Crypto)?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, key).map_err(|_| SecretError::Crypto)?);
    let aad_text = format!("{workspace_id}:{name}:{KEY_VERSION}");
    let mut in_out = ciphertext.to_vec();
    let plaintext = key
        .open_in_place(nonce, Aad::from(aad_text.as_bytes()), &mut in_out)
        .map_err(|_| SecretError::Crypto)?;
    Ok(plaintext.to_vec())
}
