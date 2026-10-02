mod common;

use hooktry::{
    hosted::{HostedRelayState, ProvisionedExposure, hosted_relay_app},
    hosted_server::HostedServerConfig,
    relay::RelayBroker,
    relay_auth::CapabilityStore,
};
use tokio::net::TcpListener;

#[test]
fn hosted_server_config_has_deployable_defaults() {
    let config = HostedServerConfig::from_lookup(|key| match key {
        "HOOKTRY_CONTROL_TOKEN" => Some("test-control-token".to_owned()),
        "HOOKTRY_SECRETS_KEY" => {
            Some("0707070707070707070707070707070707070707070707070707070707070707".to_owned())
        }
        _ => None,
    })
    .unwrap();

    assert_eq!(config.bind, "0.0.0.0:8080");
    assert_eq!(config.public_base_url, "http://127.0.0.1:8080");
    assert_eq!(config.database_url, None);
    assert_eq!(config.db_path, "hooktry-hosted.db");
}

#[test]
fn hosted_server_config_uses_port_and_public_url() {
    let config = HostedServerConfig::from_lookup(|key| match key {
        "PORT" => Some("9090".to_owned()),
        "HOOKTRY_PUBLIC_BASE_URL" => Some("https://relay.example/".to_owned()),
        "HOOKTRY_CONTROL_TOKEN" => Some("test-control-token".to_owned()),
        "HOOKTRY_DATABASE_URL" => Some("postgresql://secret@db/hooktry".to_owned()),
        "HOOKTRY_HOSTED_DB_PATH" => Some("/tmp/hooktry-hosted-test.db".to_owned()),
        "HOOKTRY_SECRETS_KEY" => {
            Some("0707070707070707070707070707070707070707070707070707070707070707".to_owned())
        }
        _ => None,
    })
    .unwrap();

    assert_eq!(config.bind, "0.0.0.0:9090");
    assert_eq!(config.public_base_url, "https://relay.example");
    assert_eq!(
        config.database_url.as_deref(),
        Some("postgresql://secret@db/hooktry")
    );
    assert_eq!(config.db_path, "/tmp/hooktry-hosted-test.db");
    let debug = format!("{config:?}");
    assert!(!debug.contains("secret@db"));
    assert!(debug.contains("[REDACTED]"));
}

#[test]
fn approval_webhook_config_is_validated_and_redacted() {
    let config = HostedServerConfig::from_lookup(|key| match key {
        "HOOKTRY_CONTROL_TOKEN" => Some("test-control-token".to_owned()),
        "HOOKTRY_SECRETS_KEY" => {
            Some("0707070707070707070707070707070707070707070707070707070707070707".to_owned())
        }
        "HOOKTRY_BOOTSTRAP_WORKSPACE" => Some("serhii".to_owned()),
        "HOOKTRY_APPROVAL_WEBHOOK_URL" => {
            Some("https://hooks.example.com/private/secret-token?key=hidden".to_owned())
        }
        _ => None,
    })
    .unwrap();

    assert_eq!(
        config.approval_webhook_url.as_deref(),
        Some("https://hooks.example.com/private/secret-token?key=hidden")
    );
    let debug = format!("{config:?}");
    assert!(!debug.contains("secret-token"));
    assert!(!debug.contains("hidden"));
    assert!(debug.contains("[REDACTED]"));

    let error = HostedServerConfig::from_lookup(|key| match key {
        "HOOKTRY_CONTROL_TOKEN" => Some("test-control-token".to_owned()),
        "HOOKTRY_SECRETS_KEY" => {
            Some("0707070707070707070707070707070707070707070707070707070707070707".to_owned())
        }
        "HOOKTRY_APPROVAL_WEBHOOK_URL" => Some("file:///tmp/hook".to_owned()),
        _ => None,
    })
    .unwrap_err();
    assert_eq!(
        error,
        "approval webhook URL must use http or https and include a host"
    );
}

#[test]
fn explicit_bind_wins_over_port() {
    let config = HostedServerConfig::from_lookup(|key| match key {
        "HOOKTRY_BIND" => Some("127.0.0.1:4242".to_owned()),
        "PORT" => Some("9090".to_owned()),
        "HOOKTRY_CONTROL_TOKEN" => Some("test-control-token".to_owned()),
        "HOOKTRY_SECRETS_KEY" => {
            Some("0707070707070707070707070707070707070707070707070707070707070707".to_owned())
        }
        _ => None,
    })
    .unwrap();

    assert_eq!(config.bind, "127.0.0.1:4242");
    assert_eq!(config.public_base_url, "http://127.0.0.1:4242");
}

#[test]
fn hosted_server_config_rejects_invalid_public_url_scheme() {
    let error = HostedServerConfig::from_lookup(|key| match key {
        "HOOKTRY_PUBLIC_BASE_URL" => Some("relay.example".to_owned()),
        "HOOKTRY_CONTROL_TOKEN" => Some("test-control-token".to_owned()),
        _ => None,
    })
    .unwrap_err();

    assert_eq!(
        error,
        "HOOKTRY_PUBLIC_BASE_URL must start with http:// or https://"
    );
}

#[tokio::test]
async fn websocket_only_hosted_app_is_healthy_and_does_not_advertise_raw_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = HostedRelayState::websocket_only(
        RelayBroker::default(),
        CapabilityStore::default(),
        format!("http://{addr}"),
        "test-control-token",
    );

    tokio::spawn(async move {
        axum::serve(listener, hosted_relay_app(state))
            .await
            .unwrap();
    });

    let health: serde_json::Value = reqwest::get(format!("http://{addr}/healthz"))
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(health["ok"], true);
    assert_eq!(health["service"], "hosted_relay");

    let client = reqwest::Client::new();
    let unauthorized = client
        .post(format!("http://{addr}/_hooktry/hosted/exposures"))
        .json(&serde_json::json!({
            "name": "app",
            "target_port": 3000
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);
    let unauthorized_body: serde_json::Value = unauthorized.json().await.unwrap();
    assert_eq!(unauthorized_body["error"]["code"], "unauthorized");

    let credential = common::issue_full_access_token(&format!("http://{addr}")).await;
    let provision: ProvisionedExposure = client
        .post(format!("http://{addr}/_hooktry/hosted/exposures"))
        .bearer_auth(&credential.token)
        .json(&serde_json::json!({
            "name": "app",
            "target_port": 3000
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    assert_eq!(provision.relay_addr, None);
    assert_eq!(
        provision.runtime_url,
        format!("ws://{addr}/_hooktry/runtime/{}", provision.exposure_id)
    );
    assert_eq!(
        provision.public_url,
        format!("http://{addr}/e/{}", provision.exposure_id)
    );
}

#[test]
fn hosted_server_config_requires_control_token() {
    let error = HostedServerConfig::from_lookup(|_| None).unwrap_err();
    assert_eq!(error, "HOOKTRY_CONTROL_TOKEN is required");
}

#[test]
fn hosted_server_config_rejects_non_postgres_database_url() {
    let error = HostedServerConfig::from_lookup(|key| match key {
        "HOOKTRY_CONTROL_TOKEN" => Some("test-control-token".to_owned()),
        "HOOKTRY_DATABASE_URL" => Some("mysql://db/hooktry".to_owned()),
        _ => None,
    })
    .unwrap_err();

    assert_eq!(error, "HOOKTRY_DATABASE_URL must be a PostgreSQL URL");
}
