use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, RwLock},
};

use postgres::{Client, NoTls};
use rand::RngCore;
use reqwest::Url;
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use rusqlite::{Connection, OptionalExtension, params};

use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretRef {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub allowed_origin: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretMetadata {
    pub reference: SecretRef,
    pub key_version: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretError {
    InvalidName,
    InvalidOrigin,
    InvalidKey,
    NotFound,
    DestinationDenied,
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
                    ON hosted_secrets(workspace_id);
                CREATE TABLE IF NOT EXISTS hosted_secret_bindings (
                    workspace_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    allowed_origin TEXT NOT NULL,
                    PRIMARY KEY(workspace_id, name)
                );",
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
                    ON hosted_secrets(workspace_id);
                CREATE TABLE IF NOT EXISTS hosted_secret_bindings (
                    workspace_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    allowed_origin TEXT NOT NULL,
                    PRIMARY KEY(workspace_id, name)
                );",
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
        self.rotate(workspace_id, name, value)
    }

    pub fn rotate(
        &self,
        workspace_id: Uuid,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<SecretRef, SecretError> {
        let name = name.into();
        let allowed_origin = self
            .find(workspace_id, &name)?
            .and_then(|secret| secret.reference.allowed_origin);
        self.rotate_with_origin(workspace_id, name, value.into(), allowed_origin)
    }

    pub fn put_bound(
        &self,
        workspace_id: Uuid,
        name: impl Into<String>,
        value: impl Into<String>,
        allowed_origin: impl Into<String>,
    ) -> Result<SecretRef, SecretError> {
        self.rotate_bound(workspace_id, name, value, allowed_origin)
    }

    pub fn rotate_bound(
        &self,
        workspace_id: Uuid,
        name: impl Into<String>,
        value: impl Into<String>,
        allowed_origin: impl Into<String>,
    ) -> Result<SecretRef, SecretError> {
        let allowed_origin = normalize_http_origin(&allowed_origin.into())?;
        self.rotate_with_origin(
            workspace_id,
            name.into(),
            value.into(),
            Some(allowed_origin),
        )
    }

    fn rotate_with_origin(
        &self,
        workspace_id: Uuid,
        name: String,
        value: String,
        allowed_origin: Option<String>,
    ) -> Result<SecretRef, SecretError> {
        if name.trim().is_empty() {
            return Err(SecretError::InvalidName);
        }
        let reference = SecretRef {
            id: Uuid::now_v7(),
            workspace_id,
            name: name.clone(),
            allowed_origin,
        };
        let envelope = encrypt(self.key.as_ref(), workspace_id, &name, value.as_bytes())?;
        self.save(StoredSecret {
            reference: reference.clone(),
            envelope,
            key_version: KEY_VERSION,
        })?;
        Ok(reference)
    }

    pub fn resolve(&self, workspace_id: Uuid, name: &str) -> Result<String, SecretError> {
        let secret = self
            .find(workspace_id, name)?
            .ok_or(SecretError::NotFound)?;
        self.decrypt_secret(workspace_id, name, &secret)
    }

    pub fn resolve_for_origin(
        &self,
        workspace_id: Uuid,
        name: &str,
        destination_origin: &str,
    ) -> Result<String, SecretError> {
        let destination_origin = normalize_http_origin(destination_origin)?;
        let secret = self
            .find(workspace_id, name)?
            .ok_or(SecretError::NotFound)?;
        if secret.reference.allowed_origin.as_deref() != Some(destination_origin.as_str()) {
            return Err(SecretError::DestinationDenied);
        }
        self.decrypt_secret(workspace_id, name, &secret)
    }

    pub fn resolve_legacy_for_origin(
        &self,
        workspace_id: Uuid,
        name: &str,
        destination_origin: &str,
    ) -> Result<String, SecretError> {
        let destination_origin = normalize_http_origin(destination_origin)?;
        let secret = self
            .find(workspace_id, name)?
            .ok_or(SecretError::NotFound)?;
        if let Some(allowed_origin) = secret.reference.allowed_origin.as_deref() {
            if allowed_origin != destination_origin {
                return Err(SecretError::DestinationDenied);
            }
        }
        self.decrypt_secret(workspace_id, name, &secret)
    }

    pub fn bind_origin(
        &self,
        workspace_id: Uuid,
        name: &str,
        allowed_origin: &str,
    ) -> Result<SecretRef, SecretError> {
        let allowed_origin = normalize_http_origin(allowed_origin)?;
        let mut secret = self
            .find(workspace_id, name)?
            .ok_or(SecretError::NotFound)?;
        secret.reference.allowed_origin = Some(allowed_origin);
        let reference = secret.reference.clone();
        self.save(secret)?;
        Ok(reference)
    }

    fn decrypt_secret(
        &self,
        workspace_id: Uuid,
        name: &str,
        secret: &StoredSecret,
    ) -> Result<String, SecretError> {
        if secret.key_version != KEY_VERSION {
            return Err(SecretError::InvalidKey);
        }
        let bytes = decrypt(self.key.as_ref(), workspace_id, name, &secret.envelope)?;
        String::from_utf8(bytes).map_err(|_| SecretError::Crypto)
    }

    pub async fn put_async(
        &self,
        workspace_id: Uuid,
        name: String,
        value: String,
    ) -> Result<SecretRef, SecretError> {
        self.rotate_async(workspace_id, name, value).await
    }

    pub async fn rotate_async(
        &self,
        workspace_id: Uuid,
        name: String,
        value: String,
    ) -> Result<SecretRef, SecretError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.rotate(workspace_id, name, value))
            .await
            .map_err(|error| SecretError::Storage(format!("join secret rotate: {error}")))?
    }

    pub async fn put_bound_async(
        &self,
        workspace_id: Uuid,
        name: String,
        value: String,
        allowed_origin: String,
    ) -> Result<SecretRef, SecretError> {
        self.rotate_bound_async(workspace_id, name, value, allowed_origin)
            .await
    }

    pub async fn rotate_bound_async(
        &self,
        workspace_id: Uuid,
        name: String,
        value: String,
        allowed_origin: String,
    ) -> Result<SecretRef, SecretError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.rotate_bound(workspace_id, name, value, allowed_origin)
        })
        .await
        .map_err(|error| SecretError::Storage(format!("join bound secret rotate: {error}")))?
    }

    pub async fn resolve_async(
        &self,
        workspace_id: Uuid,
        name: String,
    ) -> Result<String, SecretError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.resolve(workspace_id, &name))
            .await
            .map_err(|error| SecretError::Storage(format!("join secret resolve: {error}")))?
    }

    pub async fn resolve_for_origin_async(
        &self,
        workspace_id: Uuid,
        name: String,
        destination_origin: String,
    ) -> Result<String, SecretError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.resolve_for_origin(workspace_id, &name, &destination_origin)
        })
        .await
        .map_err(|error| SecretError::Storage(format!("join bound secret resolve: {error}")))?
    }

    pub async fn resolve_legacy_for_origin_async(
        &self,
        workspace_id: Uuid,
        name: String,
        destination_origin: String,
    ) -> Result<String, SecretError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.resolve_legacy_for_origin(workspace_id, &name, &destination_origin)
        })
        .await
        .map_err(|error| SecretError::Storage(format!("join legacy secret resolve: {error}")))?
    }

    pub async fn delete_async(
        &self,
        workspace_id: Uuid,
        name: String,
    ) -> Result<bool, SecretError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.delete(workspace_id, &name))
            .await
            .map_err(|error| SecretError::Storage(format!("join secret delete: {error}")))?
    }

    pub async fn list_metadata_async(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<SecretMetadata>, SecretError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.list_metadata(workspace_id))
            .await
            .map_err(|error| SecretError::Storage(format!("join secret metadata list: {error}")))?
    }

    pub fn metadata(
        &self,
        workspace_id: Uuid,
        name: &str,
    ) -> Result<Option<SecretMetadata>, SecretError> {
        Ok(self.find(workspace_id, name)?.map(|secret| SecretMetadata {
            reference: secret.reference,
            key_version: secret.key_version,
        }))
    }

    pub fn list_metadata(&self, workspace_id: Uuid) -> Result<Vec<SecretMetadata>, SecretError> {
        let mut metadata = match &self.backend {
            SecretBackend::Memory(inner) => inner
                .read()
                .expect("secret store poisoned")
                .values()
                .filter(|secret| secret.reference.workspace_id == workspace_id)
                .map(|secret| SecretMetadata {
                    reference: secret.reference.clone(),
                    key_version: secret.key_version,
                })
                .collect(),
            SecretBackend::Sqlite(connection) => {
                let connection = connection.lock().expect("secret store poisoned");
                let mut statement = connection
                    .prepare(
                        "SELECT s.secret_id,s.name,s.key_version,b.allowed_origin
                         FROM hosted_secrets s
                         LEFT JOIN hosted_secret_bindings b
                           ON b.workspace_id=s.workspace_id AND b.name=s.name
                         WHERE s.workspace_id=?1 ORDER BY s.name",
                    )
                    .map_err(|error| SecretError::Storage(error.to_string()))?;
                let rows = statement
                    .query_map(params![workspace_id.to_string()], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i32>(2)?,
                            row.get::<_, Option<String>>(3)?,
                        ))
                    })
                    .map_err(|error| SecretError::Storage(error.to_string()))?;
                let mut metadata = Vec::new();
                for row in rows {
                    let (id, name, key_version, allowed_origin) =
                        row.map_err(|error| SecretError::Storage(error.to_string()))?;
                    metadata.push(SecretMetadata {
                        reference: SecretRef {
                            id: id.parse().map_err(|error| {
                                SecretError::Storage(format!("invalid secret id: {error}"))
                            })?,
                            workspace_id,
                            name,
                            allowed_origin,
                        },
                        key_version,
                    });
                }
                metadata
            }
            SecretBackend::Postgres(client) => {
                let workspace = workspace_id.to_string();
                client
                    .lock()
                    .expect("secret store poisoned")
                    .query(
                        "SELECT s.secret_id,s.name,s.key_version,b.allowed_origin
                         FROM hosted_secrets s
                         LEFT JOIN hosted_secret_bindings b
                           ON b.workspace_id=s.workspace_id AND b.name=s.name
                         WHERE s.workspace_id=$1 ORDER BY s.name",
                        &[&workspace],
                    )
                    .map_err(|error| SecretError::Storage(error.to_string()))?
                    .into_iter()
                    .map(|row| {
                        Ok(SecretMetadata {
                            reference: SecretRef {
                                id: row.get::<_, String>(0).parse().map_err(|error| {
                                    SecretError::Storage(format!("invalid secret id: {error}"))
                                })?,
                                workspace_id,
                                name: row.get(1),
                                allowed_origin: row.get(3),
                            },
                            key_version: row.get(2),
                        })
                    })
                    .collect::<Result<Vec<_>, SecretError>>()?
            }
        };
        metadata.sort_by(|left, right| left.reference.name.cmp(&right.reference.name));
        Ok(metadata)
    }

    pub fn delete(&self, workspace_id: Uuid, name: &str) -> Result<bool, SecretError> {
        match &self.backend {
            SecretBackend::Memory(inner) => Ok(inner
                .write()
                .expect("secret store poisoned")
                .remove(&(workspace_id, name.to_owned()))
                .is_some()),
            SecretBackend::Sqlite(connection) => {
                let connection = connection.lock().expect("secret store poisoned");
                let deleted = connection
                    .execute(
                        "DELETE FROM hosted_secrets WHERE workspace_id=?1 AND name=?2",
                        params![workspace_id.to_string(), name],
                    )
                    .map_err(|error| SecretError::Storage(error.to_string()))?;
                connection
                    .execute(
                        "DELETE FROM hosted_secret_bindings WHERE workspace_id=?1 AND name=?2",
                        params![workspace_id.to_string(), name],
                    )
                    .map_err(|error| SecretError::Storage(error.to_string()))?;
                Ok(deleted > 0)
            }
            SecretBackend::Postgres(client) => {
                let workspace = workspace_id.to_string();
                let mut client = client.lock().expect("secret store poisoned");
                let deleted = client
                    .execute(
                        "DELETE FROM hosted_secrets WHERE workspace_id=$1 AND name=$2",
                        &[&workspace, &name],
                    )
                    .map_err(|error| SecretError::Storage(error.to_string()))?;
                client
                    .execute(
                        "DELETE FROM hosted_secret_bindings WHERE workspace_id=$1 AND name=$2",
                        &[&workspace, &name],
                    )
                    .map_err(|error| SecretError::Storage(error.to_string()))?;
                Ok(deleted > 0)
            }
        }
    }

    pub fn get_ref(&self, workspace_id: Uuid, name: &str) -> Option<SecretRef> {
        self.metadata(workspace_id, name)
            .ok()
            .flatten()
            .map(|metadata| metadata.reference)
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
                let connection = connection.lock().expect("secret store poisoned");
                connection
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
                match &secret.reference.allowed_origin {
                    Some(origin) => {
                        connection
                            .execute(
                                "INSERT INTO hosted_secret_bindings
                                    (workspace_id, name, allowed_origin)
                                 VALUES (?1, ?2, ?3)
                                 ON CONFLICT(workspace_id, name) DO UPDATE SET
                                    allowed_origin=excluded.allowed_origin",
                                params![
                                    secret.reference.workspace_id.to_string(),
                                    &secret.reference.name,
                                    origin
                                ],
                            )
                            .map_err(|error| SecretError::Storage(error.to_string()))?;
                    }
                    None => {
                        connection
                            .execute(
                                "DELETE FROM hosted_secret_bindings
                                 WHERE workspace_id=?1 AND name=?2",
                                params![
                                    secret.reference.workspace_id.to_string(),
                                    &secret.reference.name
                                ],
                            )
                            .map_err(|error| SecretError::Storage(error.to_string()))?;
                    }
                }
                Ok(())
            }
            SecretBackend::Postgres(client) => {
                let id = secret.reference.id.to_string();
                let workspace = secret.reference.workspace_id.to_string();
                let mut client = client.lock().expect("secret store poisoned");
                client
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
                match &secret.reference.allowed_origin {
                    Some(origin) => {
                        client
                            .execute(
                                "INSERT INTO hosted_secret_bindings
                                    (workspace_id, name, allowed_origin)
                                 VALUES ($1,$2,$3)
                                 ON CONFLICT(workspace_id, name) DO UPDATE SET
                                    allowed_origin=EXCLUDED.allowed_origin",
                                &[&workspace, &secret.reference.name, origin],
                            )
                            .map_err(|error| SecretError::Storage(error.to_string()))?;
                    }
                    None => {
                        client
                            .execute(
                                "DELETE FROM hosted_secret_bindings
                                 WHERE workspace_id=$1 AND name=$2",
                                &[&workspace, &secret.reference.name],
                            )
                            .map_err(|error| SecretError::Storage(error.to_string()))?;
                    }
                }
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
                        "SELECT s.secret_id,s.envelope,s.key_version,b.allowed_origin
                         FROM hosted_secrets s
                         LEFT JOIN hosted_secret_bindings b
                           ON b.workspace_id=s.workspace_id AND b.name=s.name
                         WHERE s.workspace_id=?1 AND s.name=?2",
                        params![workspace_id.to_string(), name],
                        |row| {
                            Ok((
                                row.get::<_, String>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, i32>(2)?,
                                row.get::<_, Option<String>>(3)?,
                            ))
                        },
                    )
                    .optional()
                    .map_err(|error| SecretError::Storage(error.to_string()))?;
                row.map(|(id, envelope, key_version, allowed_origin)| {
                    Ok(StoredSecret {
                        reference: SecretRef {
                            id: id.parse().map_err(|error| {
                                SecretError::Storage(format!("invalid secret id: {error}"))
                            })?,
                            workspace_id,
                            name: name.to_owned(),
                            allowed_origin,
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
                        "SELECT s.secret_id,s.envelope,s.key_version,b.allowed_origin
                         FROM hosted_secrets s
                         LEFT JOIN hosted_secret_bindings b
                           ON b.workspace_id=s.workspace_id AND b.name=s.name
                         WHERE s.workspace_id=$1 AND s.name=$2",
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
                            allowed_origin: row.get(3),
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

fn normalize_http_origin(value: &str) -> Result<String, SecretError> {
    let url = Url::parse(value).map_err(|_| SecretError::InvalidOrigin)?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err(SecretError::InvalidOrigin);
    }
    Ok(url.origin().ascii_serialization())
}

pub fn decode_master_key(value: &str) -> Result<[u8; 32], SecretError> {
    let bytes = hex_decode(value).map_err(|_| SecretError::InvalidKey)?;
    bytes.try_into().map_err(|_| SecretError::InvalidKey)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn hex_decode(value: &str) -> Result<Vec<u8>, ()> {
    if !value.len().is_multiple_of(2) {
        return Err(());
    }
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let high = hex_nibble(pair[0])?;
            let low = hex_nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn hex_nibble(value: u8) -> Result<u8, ()> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(()),
    }
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
    Ok(hex_encode(&envelope))
}

fn decrypt(
    key: &[u8; 32],
    workspace_id: Uuid,
    name: &str,
    envelope: &str,
) -> Result<Vec<u8>, SecretError> {
    let envelope = hex_decode(envelope).map_err(|_| SecretError::Crypto)?;
    if envelope.len() < 12 + AES_256_GCM.tag_len() {
        return Err(SecretError::Crypto);
    }
    let (nonce_bytes, ciphertext) = envelope.split_at(12);
    let nonce_bytes: [u8; 12] = nonce_bytes.try_into().map_err(|_| SecretError::Crypto)?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let key =
        LessSafeKey::new(UnboundKey::new(&AES_256_GCM, key).map_err(|_| SecretError::Crypto)?);
    let aad_text = format!("{workspace_id}:{name}:{KEY_VERSION}");
    let mut in_out = ciphertext.to_vec();
    let plaintext = key
        .open_in_place(nonce, Aad::from(aad_text.as_bytes()), &mut in_out)
        .map_err(|_| SecretError::Crypto)?;
    Ok(plaintext.to_vec())
}
