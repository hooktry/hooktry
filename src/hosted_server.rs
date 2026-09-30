use std::net::{IpAddr, SocketAddr};

use serde::Serialize;
use tokio::net::TcpListener;

use crate::{
    hosted::{HostedRelayState, hosted_relay_app},
    hosted_identity::{HostedIdentityStore, IdentityError},
    hosted_state::HostedExposureStore,
    relay::RelayBroker,
    relay_auth::CapabilityStore,
    secret::{SecretStore, decode_master_key},
};

#[derive(Clone, PartialEq, Eq)]
pub struct HostedServerConfig {
    pub bind: String,
    pub public_base_url: String,
    pub control_token: String,
    pub database_url: Option<String>,
    pub db_path: String,
    pub secrets_key: [u8; 32],
    pub bootstrap_workspace: Option<String>,
}

impl std::fmt::Debug for HostedServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostedServerConfig")
            .field("bind", &self.bind)
            .field("public_base_url", &self.public_base_url)
            .field("control_token", &"[REDACTED]")
            .field(
                "database_url",
                &self.database_url.as_ref().map(|_| "[REDACTED]"),
            )
            .field("db_path", &self.db_path)
            .field("secrets_key", &"[REDACTED]")
            .field("bootstrap_workspace", &self.bootstrap_workspace)
            .finish()
    }
}

impl HostedServerConfig {
    pub fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Result<Self, String> {
        let bind = match lookup("ORTYO_BIND") {
            Some(bind) => bind,
            None => match lookup("PORT") {
                Some(port) => format!("0.0.0.0:{}", parse_port(&port)?),
                None => "0.0.0.0:8080".to_owned(),
            },
        };

        let socket: SocketAddr = bind
            .parse()
            .map_err(|_| format!("invalid ORTYO_BIND socket address: {bind}"))?;

        let public_base_url =
            lookup("ORTYO_PUBLIC_BASE_URL").unwrap_or_else(|| local_public_base_url(socket));
        if !public_base_url.starts_with("http://") && !public_base_url.starts_with("https://") {
            return Err("ORTYO_PUBLIC_BASE_URL must start with http:// or https://".to_owned());
        }

        let control_token = lookup("ORTYO_CONTROL_TOKEN")
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| "ORTYO_CONTROL_TOKEN is required".to_owned())?;
        let database_url = lookup("ORTYO_DATABASE_URL").filter(|url| !url.trim().is_empty());
        if database_url
            .as_ref()
            .is_some_and(|url| !url.starts_with("postgres://") && !url.starts_with("postgresql://"))
        {
            return Err("ORTYO_DATABASE_URL must be a PostgreSQL URL".to_owned());
        }
        let secrets_key = lookup("ORTYO_SECRETS_KEY")
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "ORTYO_SECRETS_KEY is required".to_owned())
            .and_then(|value| {
                decode_master_key(&value).map_err(|_| {
                    "ORTYO_SECRETS_KEY must be exactly 64 hexadecimal characters".to_owned()
                })
            })?;
        let bootstrap_workspace =
            lookup("ORTYO_BOOTSTRAP_WORKSPACE").filter(|value| !value.trim().is_empty());
        let db_path = lookup("ORTYO_HOSTED_DB_PATH")
            .filter(|path| !path.trim().is_empty())
            .unwrap_or_else(|| "ortyo-hosted.db".to_owned());

        Ok(Self {
            bind,
            public_base_url: public_base_url.trim_end_matches('/').to_owned(),
            control_token,
            database_url,
            db_path,
            secrets_key,
            bootstrap_workspace,
        })
    }
}

#[derive(Debug, Serialize)]
struct HostedStartup {
    service: &'static str,
    bind: String,
    public_base_url: String,
    runtime_transport: &'static str,
    storage: &'static str,
}

pub async fn run_hosted_server(config: HostedServerConfig) -> Result<(), String> {
    let listener = TcpListener::bind(&config.bind)
        .await
        .map_err(|error| format!("bind hosted relay {}: {error}", config.bind))?;

    let (capabilities, exposures, identities, secrets, storage) =
        open_hosted_stores(&config).await?;
    if let Some(slug) = config.bootstrap_workspace.as_deref() {
        ensure_operator_bootstrap(&identities, &secrets, slug)?;
    }
    let state = HostedRelayState::websocket_only_with_stores(
        RelayBroker::default(),
        capabilities,
        exposures,
        identities,
        secrets,
        config.public_base_url.clone(),
        &config.control_token,
    );

    let startup = HostedStartup {
        service: "hosted_relay",
        bind: config.bind,
        public_base_url: config.public_base_url,
        runtime_transport: "websocket",
        storage,
    };
    println!(
        "{}",
        serde_json::to_string(&startup).map_err(|error| error.to_string())?
    );

    axum::serve(listener, hosted_relay_app(state))
        .await
        .map_err(|error| format!("serve hosted relay: {error}"))
}

async fn open_hosted_stores(
    config: &HostedServerConfig,
) -> Result<
    (
        CapabilityStore,
        HostedExposureStore,
        HostedIdentityStore,
        SecretStore,
        &'static str,
    ),
    String,
> {
    if let Some(database_url) = &config.database_url {
        let database_url = database_url.clone();
        let secrets_key = config.secrets_key;
        tokio::task::spawn_blocking(move || {
            let capabilities = CapabilityStore::open_postgres(&database_url)
                .map_err(|error| format!("open Postgres capability store: {error:?}"))?;
            let exposures = HostedExposureStore::open_postgres(&database_url)
                .map_err(|error| format!("open Postgres exposure store: {error:?}"))?;
            let identities = HostedIdentityStore::open_postgres(&database_url)
                .map_err(|error| format!("open Postgres identity store: {error:?}"))?;
            let secrets = SecretStore::open_postgres(&database_url, secrets_key)
                .map_err(|error| format!("open Postgres secret store: {error:?}"))?;
            Ok((capabilities, exposures, identities, secrets, "postgres"))
        })
        .await
        .map_err(|error| format!("join Postgres store initialization: {error}"))?
    } else {
        let db_path = config.db_path.clone();
        let secrets_key = config.secrets_key;
        tokio::task::spawn_blocking(move || {
            let capabilities = CapabilityStore::open(&db_path)
                .map_err(|error| format!("open SQLite capability store: {error:?}"))?;
            let exposures = HostedExposureStore::open(&db_path)
                .map_err(|error| format!("open SQLite exposure store: {error:?}"))?;
            let identities = HostedIdentityStore::open(&db_path)
                .map_err(|error| format!("open SQLite identity store: {error:?}"))?;
            let secrets = SecretStore::open(&db_path, secrets_key)
                .map_err(|error| format!("open SQLite secret store: {error:?}"))?;
            Ok((capabilities, exposures, identities, secrets, "sqlite"))
        })
        .await
        .map_err(|error| format!("join SQLite store initialization: {error}"))?
    }
}

fn ensure_operator_bootstrap(
    identities: &HostedIdentityStore,
    secrets: &SecretStore,
    slug: &str,
) -> Result<(), String> {
    const SECRET_NAME: &str = "default-api-token";
    if let Some(workspace) = identities
        .find_workspace_by_slug(slug)
        .map_err(|error| format!("find bootstrap workspace: {error:?}"))?
    {
        if secrets.get_ref(workspace.id, SECRET_NAME).is_some() {
            return Ok(());
        }
        let credential = identities
            .issue_credential(
                workspace.id,
                "operator-bootstrap-recovery",
                &[
                    crate::hosted_identity::ApiScope::ExposuresCreate,
                    crate::hosted_identity::ApiScope::ExposuresRead,
                    crate::hosted_identity::ApiScope::ExposuresRevoke,
                    crate::hosted_identity::ApiScope::RequestsExecute,
                ],
            )
            .map_err(|error| format!("issue bootstrap recovery credential: {error:?}"))?;
        secrets
            .put(workspace.id, SECRET_NAME, credential.token)
            .map_err(|error| format!("persist bootstrap recovery credential: {error:?}"))?;
        return Ok(());
    }

    match identities.bootstrap_first_workspace(slug, "operator-bootstrap") {
        Ok((workspace, credential)) => {
            secrets
                .put(workspace.id, SECRET_NAME, credential.token)
                .map_err(|error| format!("persist bootstrap credential: {error:?}"))?;
            Ok(())
        }
        Err(IdentityError::BootstrapAlreadyCompleted) => Err(
            "bootstrap workspace does not match the already initialized hosted identity store"
                .to_owned(),
        ),
        Err(error) => Err(format!("bootstrap first workspace: {error:?}")),
    }
}

fn parse_port(value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| format!("invalid PORT: {value}"))
}

fn local_public_base_url(socket: SocketAddr) -> String {
    let host = match socket.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => "127.0.0.1".to_owned(),
        IpAddr::V6(ip) if ip.is_unspecified() => "[::1]".to_owned(),
        IpAddr::V6(ip) => format!("[{ip}]"),
        IpAddr::V4(ip) => ip.to_string(),
    };
    format!("http://{host}:{}", socket.port())
}

#[cfg(test)]
mod tests {
    use super::{HostedServerConfig, ensure_operator_bootstrap, open_hosted_stores};
    use crate::{
        hosted_identity::{ApiScope, HostedIdentityStore},
        secret::SecretStore,
    };
    use uuid::Uuid;

    #[test]
    fn operator_bootstrap_captures_token_and_is_idempotent() {
        let path = std::env::temp_dir().join(format!("ortyo-bootstrap-{}.db", Uuid::now_v7()));
        let identities = HostedIdentityStore::open(&path).unwrap();
        let secrets = SecretStore::open(&path, [41; 32]).unwrap();

        ensure_operator_bootstrap(&identities, &secrets, "serhii").unwrap();
        let workspace = identities
            .find_workspace_by_slug("serhii")
            .unwrap()
            .unwrap();
        let token = secrets.resolve(workspace.id, "default-api-token").unwrap();
        identities
            .authorize(&token, ApiScope::RequestsExecute)
            .unwrap();

        let first_ref = secrets.get_ref(workspace.id, "default-api-token").unwrap();
        ensure_operator_bootstrap(&identities, &secrets, "serhii").unwrap();
        let second_ref = secrets.get_ref(workspace.id, "default-api-token").unwrap();
        assert_eq!(first_ref.id, second_ref.id);

        drop(secrets);
        drop(identities);
        let reopened = SecretStore::open(&path, [41; 32]).unwrap();
        assert_eq!(
            reopened.resolve(workspace.id, "default-api-token").unwrap(),
            token
        );
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn postgres_initialization_fails_without_nested_runtime_panic() {
        let config = HostedServerConfig {
            bind: "127.0.0.1:0".to_owned(),
            public_base_url: "http://127.0.0.1".to_owned(),
            control_token: "test-control-token".to_owned(),
            database_url: Some("postgresql://127.0.0.1:1/ortyo".to_owned()),
            db_path: "unused.db".to_owned(),
            secrets_key: [7; 32],
            bootstrap_workspace: None,
        };

        let error = match open_hosted_stores(&config).await {
            Ok(_) => panic!("unreachable Postgres unexpectedly opened"),
            Err(error) => error,
        };

        assert!(
            error.contains("open Postgres capability store"),
            "unexpected error: {error}"
        );
    }
}
