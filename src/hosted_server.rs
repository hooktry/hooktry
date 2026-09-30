use std::net::{IpAddr, SocketAddr};

use serde::Serialize;
use tokio::net::TcpListener;

use crate::{
    hosted::{HostedRelayState, hosted_relay_app},
    relay::RelayBroker,
    relay_auth::CapabilityStore,
};

#[derive(Clone, PartialEq, Eq)]
pub struct HostedServerConfig {
    pub bind: String,
    pub public_base_url: String,
    pub control_token: String,
}

impl std::fmt::Debug for HostedServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostedServerConfig")
            .field("bind", &self.bind)
            .field("public_base_url", &self.public_base_url)
            .field("control_token", &"[REDACTED]")
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

        Ok(Self {
            bind,
            public_base_url: public_base_url.trim_end_matches('/').to_owned(),
            control_token,
        })
    }
}

#[derive(Debug, Serialize)]
struct HostedStartup {
    service: &'static str,
    bind: String,
    public_base_url: String,
    runtime_transport: &'static str,
}

pub async fn run_hosted_server(config: HostedServerConfig) -> Result<(), String> {
    let listener = TcpListener::bind(&config.bind)
        .await
        .map_err(|error| format!("bind hosted relay {}: {error}", config.bind))?;

    let state = HostedRelayState::websocket_only(
        RelayBroker::default(),
        CapabilityStore::default(),
        config.public_base_url.clone(),
        &config.control_token,
    );

    let startup = HostedStartup {
        service: "hosted_relay",
        bind: config.bind,
        public_base_url: config.public_base_url,
        runtime_transport: "websocket",
    };
    println!(
        "{}",
        serde_json::to_string(&startup).map_err(|error| error.to_string())?
    );

    axum::serve(listener, hosted_relay_app(state))
        .await
        .map_err(|error| format!("serve hosted relay: {error}"))
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
