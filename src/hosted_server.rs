use std::net::{IpAddr, SocketAddr};

use serde::Serialize;
use tokio::net::TcpListener;

use crate::{
    hosted::{HostedRelayState, hosted_relay_app},
    hosted_identity::HostedIdentityStore,
    hosted_state::HostedExposureStore,
    relay::RelayBroker,
    relay_auth::CapabilityStore,
    secret::SecretStore,
};

#[derive(Clone, PartialEq, Eq)]
pub struct HostedServerConfig {
    pub bind: String,
    pub public_base_url: String,
    pub control_token: String,
    pub database_url: Option<String>,
    pub db_path: String,
    pub secrets_key: [u8; 32],
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
        let db_path = lookup("ORTYO_HOSTED_DB_PATH")
            .filter(|path| !path.trim().is_empty())
            .unwrap_or_else(|| "ortyo-hosted.db".to_owned());

        Ok(Self {
            bind,
            public_base_url: public_base_url.trim_end_matches('/').to_owned(),
            control_token,
            database_url,
            db_path,
            secrets_key: [0; 32],
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

    let (capabilities, exposures, identities, secrets, storage) = open_hosted_stores(&config).await?;
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
    use super::{HostedServerConfig, open_hosted_stores};

    #[tokio::test]
    async fn postgres_initialization_fails_without_nested_runtime_panic() {
        let config = HostedServerConfig {
            bind: "127.0.0.1:0".to_owned(),
            public_base_url: "http://127.0.0.1".to_owned(),
            control_token: "test-control-token".to_owned(),
            database_url: Some("postgresql://127.0.0.1:1/ortyo".to_owned()),
            db_path: "unused.db".to_owned(),
            secrets_key: [7; 32],
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
