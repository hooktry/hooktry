use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex, RwLock},
};

use postgres::{Client, NoTls};
use rand::RngCore;
use ring::digest::{SHA256, digest};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Workspace {
    pub id: Uuid,
    pub slug: String,
    pub status: WorkspaceStatus,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceStatus {
    Active,
    Disabled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ApiScope {
    #[serde(rename = "exposures:create")]
    ExposuresCreate,
    #[serde(rename = "exposures:read")]
    ExposuresRead,
    #[serde(rename = "exposures:revoke")]
    ExposuresRevoke,
    #[serde(rename = "requests:execute")]
    RequestsExecute,
}

impl ApiScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::ExposuresCreate => "exposures:create",
            Self::ExposuresRead => "exposures:read",
            Self::ExposuresRevoke => "exposures:revoke",
            Self::RequestsExecute => "requests:execute",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "exposures:create" => Some(Self::ExposuresCreate),
            "exposures:read" => Some(Self::ExposuresRead),
            "exposures:revoke" => Some(Self::ExposuresRevoke),
            "requests:execute" => Some(Self::RequestsExecute),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiCredential {
    pub credential_id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub scopes: Vec<ApiScope>,
    pub revoked: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssuedApiCredential {
    pub credential_id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub scopes: Vec<ApiScope>,
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiAuthorization {
    pub credential_id: Uuid,
    pub workspace_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityError {
    InvalidWorkspace,
    DuplicateWorkspace,
    WorkspaceNotFound,
    WorkspaceDisabled,
    InvalidCredential,
    RevokedCredential,
    Forbidden,
    BootstrapAlreadyCompleted,
    Storage(String),
}

#[derive(Clone)]
enum IdentityBackend {
    Memory(Arc<RwLock<MemoryIdentity>>),
    Sqlite(Arc<Mutex<Connection>>),
    Postgres(Arc<Mutex<Client>>),
}

#[derive(Default)]
struct MemoryIdentity {
    workspaces: HashMap<Uuid, Workspace>,
    slugs: HashMap<String, Uuid>,
    credentials: HashMap<[u8; 32], ApiCredential>,
}

#[derive(Clone)]
pub struct HostedIdentityStore {
    backend: IdentityBackend,
}

impl Default for HostedIdentityStore {
    fn default() -> Self {
        Self {
            backend: IdentityBackend::Memory(Arc::new(RwLock::new(MemoryIdentity::default()))),
        }
    }
}

impl HostedIdentityStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, IdentityError> {
        let connection =
            Connection::open(path).map_err(|error| IdentityError::Storage(error.to_string()))?;
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS hosted_workspaces (
                    id TEXT PRIMARY KEY,
                    slug TEXT NOT NULL UNIQUE,
                    status TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS hosted_api_credentials (
                    token_digest BLOB PRIMARY KEY,
                    credential_id TEXT NOT NULL UNIQUE,
                    workspace_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    scopes TEXT NOT NULL,
                    revoked INTEGER NOT NULL DEFAULT 0
                );
                CREATE INDEX IF NOT EXISTS hosted_api_credentials_workspace
                    ON hosted_api_credentials(workspace_id);",
            )
            .map_err(|error| IdentityError::Storage(error.to_string()))?;
        Ok(Self {
            backend: IdentityBackend::Sqlite(Arc::new(Mutex::new(connection))),
        })
    }

    pub fn open_postgres(database_url: &str) -> Result<Self, IdentityError> {
        let mut client = Client::connect(database_url, NoTls)
            .map_err(|error| IdentityError::Storage(error.to_string()))?;
        client
            .batch_execute(
                "CREATE TABLE IF NOT EXISTS hosted_workspaces (
                    id TEXT PRIMARY KEY,
                    slug TEXT NOT NULL UNIQUE,
                    status TEXT NOT NULL
                );
                CREATE TABLE IF NOT EXISTS hosted_api_credentials (
                    token_digest BYTEA PRIMARY KEY,
                    credential_id TEXT NOT NULL UNIQUE,
                    workspace_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    scopes TEXT NOT NULL,
                    revoked BOOLEAN NOT NULL DEFAULT FALSE
                );
                CREATE INDEX IF NOT EXISTS hosted_api_credentials_workspace
                    ON hosted_api_credentials(workspace_id);",
            )
            .map_err(|error| IdentityError::Storage(error.to_string()))?;
        Ok(Self {
            backend: IdentityBackend::Postgres(Arc::new(Mutex::new(client))),
        })
    }

    pub async fn bootstrap_first_workspace_async(
        &self,
        slug: String,
        credential_name: String,
    ) -> Result<(Workspace, IssuedApiCredential), IdentityError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || {
            store.bootstrap_first_workspace(&slug, &credential_name)
        })
        .await
        .map_err(|error| IdentityError::Storage(error.to_string()))?
    }

    pub fn bootstrap_first_workspace(
        &self,
        slug: &str,
        credential_name: &str,
    ) -> Result<(Workspace, IssuedApiCredential), IdentityError> {
        let slug = normalize_slug(slug)?;
        if credential_name.trim().is_empty() {
            return Err(IdentityError::InvalidCredential);
        }

        let workspace = Workspace {
            id: Uuid::now_v7(),
            slug,
            status: WorkspaceStatus::Active,
        };
        let token = api_token();
        let credential = ApiCredential {
            credential_id: Uuid::now_v7(),
            workspace_id: workspace.id,
            name: credential_name.trim().to_owned(),
            scopes: vec![
                ApiScope::ExposuresCreate,
                ApiScope::ExposuresRead,
                ApiScope::ExposuresRevoke,
                ApiScope::RequestsExecute,
            ],
            revoked: false,
        };
        let digest = token_digest(&token);
        let scopes = encode_scopes(&credential.scopes);

        match &self.backend {
            IdentityBackend::Memory(inner) => {
                let mut inner = inner.write().expect("identity store poisoned");
                if !inner.workspaces.is_empty() {
                    return Err(IdentityError::BootstrapAlreadyCompleted);
                }
                inner.slugs.insert(workspace.slug.clone(), workspace.id);
                inner.workspaces.insert(workspace.id, workspace.clone());
                inner.credentials.insert(digest, credential.clone());
            }
            IdentityBackend::Sqlite(connection) => {
                let mut connection = connection.lock().expect("identity store poisoned");
                let transaction = connection
                    .transaction()
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
                let count: i64 = transaction
                    .query_row("SELECT COUNT(*) FROM hosted_workspaces", [], |row| {
                        row.get(0)
                    })
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
                if count != 0 {
                    return Err(IdentityError::BootstrapAlreadyCompleted);
                }
                transaction
                    .execute(
                        "INSERT INTO hosted_workspaces (id, slug, status) VALUES (?1, ?2, 'active')",
                        params![workspace.id.to_string(), &workspace.slug],
                    )
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
                transaction
                    .execute(
                        "INSERT INTO hosted_api_credentials
                            (token_digest, credential_id, workspace_id, name, scopes, revoked)
                         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
                        params![
                            digest.as_slice(),
                            credential.credential_id.to_string(),
                            workspace.id.to_string(),
                            &credential.name,
                            &scopes
                        ],
                    )
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
                transaction
                    .commit()
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
            }
            IdentityBackend::Postgres(client) => {
                let mut client = client.lock().expect("identity store poisoned");
                let mut transaction = client
                    .transaction()
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
                transaction
                    .batch_execute("LOCK TABLE hosted_workspaces IN ACCESS EXCLUSIVE MODE")
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
                let count: i64 = transaction
                    .query_one("SELECT COUNT(*) FROM hosted_workspaces", &[])
                    .map_err(|error| IdentityError::Storage(error.to_string()))?
                    .get(0);
                if count != 0 {
                    return Err(IdentityError::BootstrapAlreadyCompleted);
                }
                let workspace_id = workspace.id.to_string();
                let credential_id = credential.credential_id.to_string();
                transaction
                    .execute(
                        "INSERT INTO hosted_workspaces (id, slug, status) VALUES ($1, $2, 'active')",
                        &[&workspace_id, &workspace.slug],
                    )
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
                transaction
                    .execute(
                        "INSERT INTO hosted_api_credentials
                            (token_digest, credential_id, workspace_id, name, scopes, revoked)
                         VALUES ($1, $2, $3, $4, $5, FALSE)",
                        &[
                            &digest.as_slice(),
                            &credential_id,
                            &workspace_id,
                            &credential.name,
                            &scopes,
                        ],
                    )
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
                transaction
                    .commit()
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
            }
        }

        let issued = IssuedApiCredential {
            credential_id: credential.credential_id,
            workspace_id: workspace.id,
            name: credential.name,
            scopes: credential.scopes,
            token,
        };
        Ok((workspace, issued))
    }

    pub fn find_workspace_by_slug(&self, slug: &str) -> Result<Option<Workspace>, IdentityError> {
        let slug = normalize_slug(slug)?;
        match &self.backend {
            IdentityBackend::Memory(inner) => {
                let inner = inner.read().expect("identity store poisoned");
                Ok(inner
                    .slugs
                    .get(&slug)
                    .and_then(|id| inner.workspaces.get(id))
                    .cloned())
            }
            IdentityBackend::Sqlite(connection) => connection
                .lock()
                .expect("identity store poisoned")
                .query_row(
                    "SELECT id, status FROM hosted_workspaces WHERE slug = ?1",
                    params![slug],
                    |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    },
                )
                .optional()
                .map_err(|error| IdentityError::Storage(error.to_string()))?
                .map(|(id, status)| {
                    Ok(Workspace {
                        id: id.parse().map_err(|error| {
                            IdentityError::Storage(format!("invalid workspace id: {error}"))
                        })?,
                        slug,
                        status: parse_workspace_status(&status)?,
                    })
                })
                .transpose(),
            IdentityBackend::Postgres(client) => {
                let row = client
                    .lock()
                    .expect("identity store poisoned")
                    .query_opt(
                        "SELECT id, status FROM hosted_workspaces WHERE slug = $1",
                        &[&slug],
                    )
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
                row.map(|row| {
                    Ok(Workspace {
                        id: row.get::<_, String>(0).parse().map_err(|error| {
                            IdentityError::Storage(format!("invalid workspace id: {error}"))
                        })?,
                        slug,
                        status: parse_workspace_status(row.get::<_, String>(1).as_str())?,
                    })
                })
                .transpose()
            }
        }
    }

    pub async fn create_workspace_async(&self, slug: String) -> Result<Workspace, IdentityError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.create_workspace(&slug))
            .await
            .map_err(|error| IdentityError::Storage(error.to_string()))?
    }

    pub async fn issue_credential_async(
        &self,
        workspace_id: Uuid,
        name: String,
        scopes: Vec<ApiScope>,
    ) -> Result<IssuedApiCredential, IdentityError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.issue_credential(workspace_id, &name, &scopes))
            .await
            .map_err(|error| IdentityError::Storage(error.to_string()))?
    }

    pub async fn authorize_async(
        &self,
        token: String,
        required_scope: ApiScope,
    ) -> Result<ApiAuthorization, IdentityError> {
        let store = self.clone();
        tokio::task::spawn_blocking(move || store.authorize(&token, required_scope))
            .await
            .map_err(|error| IdentityError::Storage(error.to_string()))?
    }

    pub fn create_workspace(&self, slug: &str) -> Result<Workspace, IdentityError> {
        let slug = normalize_slug(slug)?;
        let workspace = Workspace {
            id: Uuid::now_v7(),
            slug: slug.clone(),
            status: WorkspaceStatus::Active,
        };

        match &self.backend {
            IdentityBackend::Memory(inner) => {
                let mut inner = inner.write().expect("identity store poisoned");
                if inner.slugs.contains_key(&slug) {
                    return Err(IdentityError::DuplicateWorkspace);
                }
                inner.slugs.insert(slug, workspace.id);
                inner.workspaces.insert(workspace.id, workspace.clone());
            }
            IdentityBackend::Sqlite(connection) => {
                connection
                    .lock()
                    .expect("identity store poisoned")
                    .execute(
                        "INSERT INTO hosted_workspaces (id, slug, status) VALUES (?1, ?2, ?3)",
                        params![workspace.id.to_string(), &workspace.slug, "active"],
                    )
                    .map_err(|error| {
                        if error.to_string().contains("UNIQUE") {
                            IdentityError::DuplicateWorkspace
                        } else {
                            IdentityError::Storage(error.to_string())
                        }
                    })?;
            }
            IdentityBackend::Postgres(client) => {
                let id = workspace.id.to_string();
                client
                    .lock()
                    .expect("identity store poisoned")
                    .execute(
                        "INSERT INTO hosted_workspaces (id, slug, status) VALUES ($1, $2, $3)",
                        &[&id, &workspace.slug, &"active"],
                    )
                    .map_err(|error| {
                        if error.code() == Some(&postgres::error::SqlState::UNIQUE_VIOLATION) {
                            IdentityError::DuplicateWorkspace
                        } else {
                            IdentityError::Storage(error.to_string())
                        }
                    })?;
            }
        }

        Ok(workspace)
    }

    pub fn issue_credential(
        &self,
        workspace_id: Uuid,
        name: &str,
        scopes: &[ApiScope],
    ) -> Result<IssuedApiCredential, IdentityError> {
        if name.trim().is_empty() || scopes.is_empty() {
            return Err(IdentityError::InvalidCredential);
        }
        self.require_active_workspace(workspace_id)?;

        let token = api_token();
        let credential = ApiCredential {
            credential_id: Uuid::now_v7(),
            workspace_id,
            name: name.trim().to_owned(),
            scopes: canonical_scopes(scopes),
            revoked: false,
        };
        let encoded_scopes = encode_scopes(&credential.scopes);
        let token_digest = token_digest(&token);

        match &self.backend {
            IdentityBackend::Memory(inner) => {
                inner
                    .write()
                    .expect("identity store poisoned")
                    .credentials
                    .insert(token_digest, credential.clone());
            }
            IdentityBackend::Sqlite(connection) => {
                connection
                    .lock()
                    .expect("identity store poisoned")
                    .execute(
                        "INSERT INTO hosted_api_credentials
                            (token_digest, credential_id, workspace_id, name, scopes, revoked)
                         VALUES (?1, ?2, ?3, ?4, ?5, 0)",
                        params![
                            token_digest.as_slice(),
                            credential.credential_id.to_string(),
                            workspace_id.to_string(),
                            &credential.name,
                            encoded_scopes
                        ],
                    )
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
            }
            IdentityBackend::Postgres(client) => {
                let credential_id = credential.credential_id.to_string();
                let workspace_id = workspace_id.to_string();
                client
                    .lock()
                    .expect("identity store poisoned")
                    .execute(
                        "INSERT INTO hosted_api_credentials
                            (token_digest, credential_id, workspace_id, name, scopes, revoked)
                         VALUES ($1, $2, $3, $4, $5, FALSE)",
                        &[
                            &token_digest.as_slice(),
                            &credential_id,
                            &workspace_id,
                            &credential.name,
                            &encoded_scopes,
                        ],
                    )
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
            }
        }

        Ok(IssuedApiCredential {
            credential_id: credential.credential_id,
            workspace_id,
            name: credential.name,
            scopes: credential.scopes,
            token,
        })
    }

    pub fn authorize(
        &self,
        token: &str,
        required_scope: ApiScope,
    ) -> Result<ApiAuthorization, IdentityError> {
        let credential = self
            .find_credential(token_digest(token))?
            .ok_or(IdentityError::InvalidCredential)?;
        if credential.revoked {
            return Err(IdentityError::RevokedCredential);
        }
        self.require_active_workspace(credential.workspace_id)?;
        if !credential.scopes.contains(&required_scope) {
            return Err(IdentityError::Forbidden);
        }
        Ok(ApiAuthorization {
            credential_id: credential.credential_id,
            workspace_id: credential.workspace_id,
        })
    }

    fn require_active_workspace(&self, workspace_id: Uuid) -> Result<(), IdentityError> {
        let workspace = self
            .find_workspace(workspace_id)?
            .ok_or(IdentityError::WorkspaceNotFound)?;
        if workspace.status != WorkspaceStatus::Active {
            return Err(IdentityError::WorkspaceDisabled);
        }
        Ok(())
    }

    fn find_workspace(&self, workspace_id: Uuid) -> Result<Option<Workspace>, IdentityError> {
        match &self.backend {
            IdentityBackend::Memory(inner) => Ok(inner
                .read()
                .expect("identity store poisoned")
                .workspaces
                .get(&workspace_id)
                .cloned()),
            IdentityBackend::Sqlite(connection) => {
                let row = connection
                    .lock()
                    .expect("identity store poisoned")
                    .query_row(
                        "SELECT slug, status FROM hosted_workspaces WHERE id = ?1",
                        [workspace_id.to_string()],
                        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                    )
                    .optional()
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
                row.map(|(slug, status)| {
                    Ok(Workspace {
                        id: workspace_id,
                        slug,
                        status: parse_workspace_status(&status)?,
                    })
                })
                .transpose()
            }
            IdentityBackend::Postgres(client) => {
                let id = workspace_id.to_string();
                let row = client
                    .lock()
                    .expect("identity store poisoned")
                    .query_opt(
                        "SELECT slug, status FROM hosted_workspaces WHERE id = $1",
                        &[&id],
                    )
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
                row.map(|row| {
                    Ok(Workspace {
                        id: workspace_id,
                        slug: row.get(0),
                        status: parse_workspace_status(row.get::<_, String>(1).as_str())?,
                    })
                })
                .transpose()
            }
        }
    }

    fn find_credential(
        &self,
        token_digest: [u8; 32],
    ) -> Result<Option<ApiCredential>, IdentityError> {
        match &self.backend {
            IdentityBackend::Memory(inner) => Ok(inner
                .read()
                .expect("identity store poisoned")
                .credentials
                .get(&token_digest)
                .cloned()),
            IdentityBackend::Sqlite(connection) => connection
                .lock()
                .expect("identity store poisoned")
                .query_row(
                    "SELECT credential_id, workspace_id, name, scopes, revoked
                     FROM hosted_api_credentials
                     WHERE token_digest = ?1",
                    params![token_digest.as_slice()],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, bool>(4)?,
                        ))
                    },
                )
                .optional()
                .map_err(|error| IdentityError::Storage(error.to_string()))?
                .map(api_credential_from_raw)
                .transpose(),
            IdentityBackend::Postgres(client) => {
                let row = client
                    .lock()
                    .expect("identity store poisoned")
                    .query_opt(
                        "SELECT credential_id, workspace_id, name, scopes, revoked
                         FROM hosted_api_credentials
                         WHERE token_digest = $1",
                        &[&token_digest.as_slice()],
                    )
                    .map_err(|error| IdentityError::Storage(error.to_string()))?;
                row.map(|row| {
                    api_credential_from_raw((
                        row.get::<_, String>(0),
                        row.get::<_, String>(1),
                        row.get::<_, String>(2),
                        row.get::<_, String>(3),
                        row.get::<_, bool>(4),
                    ))
                })
                .transpose()
            }
        }
    }
}

fn normalize_slug(value: &str) -> Result<String, IdentityError> {
    let slug = value.trim().to_ascii_lowercase();
    if slug.is_empty()
        || slug.len() > 63
        || !slug
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || slug.starts_with('-')
        || slug.ends_with('-')
    {
        return Err(IdentityError::InvalidWorkspace);
    }
    Ok(slug)
}

fn canonical_scopes(scopes: &[ApiScope]) -> Vec<ApiScope> {
    [
        ApiScope::ExposuresCreate,
        ApiScope::ExposuresRead,
        ApiScope::ExposuresRevoke,
        ApiScope::RequestsExecute,
    ]
    .into_iter()
    .filter(|scope| scopes.contains(scope))
    .collect()
}

fn encode_scopes(scopes: &[ApiScope]) -> String {
    scopes
        .iter()
        .map(|scope| scope.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

fn decode_scopes(value: &str) -> Result<Vec<ApiScope>, IdentityError> {
    value
        .split(',')
        .filter(|value| !value.is_empty())
        .map(|value| {
            ApiScope::parse(value)
                .ok_or_else(|| IdentityError::Storage(format!("invalid API scope: {value}")))
        })
        .collect()
}

fn api_credential_from_raw(
    (credential_id, workspace_id, name, scopes, revoked): (String, String, String, String, bool),
) -> Result<ApiCredential, IdentityError> {
    Ok(ApiCredential {
        credential_id: credential_id
            .parse()
            .map_err(|error| IdentityError::Storage(format!("invalid credential id: {error}")))?,
        workspace_id: workspace_id
            .parse()
            .map_err(|error| IdentityError::Storage(format!("invalid workspace id: {error}")))?,
        name,
        scopes: decode_scopes(&scopes)?,
        revoked,
    })
}

fn parse_workspace_status(value: &str) -> Result<WorkspaceStatus, IdentityError> {
    match value {
        "active" => Ok(WorkspaceStatus::Active),
        "disabled" => Ok(WorkspaceStatus::Disabled),
        _ => Err(IdentityError::Storage(format!(
            "invalid workspace status: {value}"
        ))),
    }
}

fn token_digest(token: &str) -> [u8; 32] {
    let digest = digest(&SHA256, token.as_bytes());
    digest
        .as_ref()
        .try_into()
        .expect("SHA-256 digest is always 32 bytes")
}

fn api_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut encoded, "{byte:02x}").expect("write to String cannot fail");
    }

    format!("ortyo_{}", encoded)
}
