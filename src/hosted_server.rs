use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    time::{Duration, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::json;
use tokio::{net::TcpListener, time::sleep};
use uuid::Uuid;

use crate::{
    anonymous::AnonymousExposureStore,
    approval::{
        ApprovalNotificationEvent, ApprovalRecord, ApprovalState, ApprovalStore,
        KEYED_FINGERPRINT_PREFIX, derive_digest_keyring,
    },
    approval_webhook::{
        ensure_webhook_secret, run_worker, validate_webhook_url, webhook_headers, webhook_payload,
    },
    domain::{ExposureAccess, ExposureMode},
    execution::{
        ExecutionError, ExecutionOutcome, HttpExecutionRequest, SecretCapture, SecretHeaderBinding,
    },
    execution_store::{DurableExecutionState, ExecutionStore},
    hosted::{ApprovedExecution, HostedRelayState, ProvisionedExposure, hosted_relay_app},
    hosted_identity::{HostedIdentityStore, IdentityError},
    hosted_state::{HostedExposureRecord, HostedExposureStore},
    http::AppState,
    keyring::{VersionedKeyring, VersionedKeyringError},
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
    pub secrets_keyring: VersionedKeyring,
    pub bootstrap_workspace: Option<String>,
    pub approval_webhook_url: Option<String>,
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
            .field("secrets_keyring", &"[REDACTED]")
            .field("bootstrap_workspace", &self.bootstrap_workspace)
            .field(
                "approval_webhook_url",
                &self.approval_webhook_url.as_ref().map(|_| "[REDACTED]"),
            )
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
        let secrets_key_version = lookup("ORTYO_SECRETS_KEY_VERSION")
            .filter(|value| !value.trim().is_empty())
            .map(|value| parse_positive_key_version("ORTYO_SECRETS_KEY_VERSION", &value))
            .transpose()?
            .unwrap_or(1);
        let previous_secrets_keys = lookup("ORTYO_SECRETS_PREVIOUS_KEYS")
            .filter(|value| !value.trim().is_empty())
            .map(|value| parse_previous_secret_keys(&value))
            .transpose()?
            .unwrap_or_default();
        let secrets_keyring =
            VersionedKeyring::new(secrets_key_version, secrets_key, previous_secrets_keys)
                .map_err(keyring_config_error)?;
        let bootstrap_workspace =
            lookup("ORTYO_BOOTSTRAP_WORKSPACE").filter(|value| !value.trim().is_empty());
        let approval_webhook_url =
            lookup("ORTYO_APPROVAL_WEBHOOK_URL").filter(|value| !value.trim().is_empty());
        if let Some(url) = approval_webhook_url.as_deref() {
            validate_webhook_url(url)?;
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
            secrets_keyring,
            bootstrap_workspace,
            approval_webhook_url,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DogfoodExposureConfig {
    name: String,
    target_port: u16,
}

#[derive(Debug, Serialize)]
struct HostedStartup {
    service: &'static str,
    bind: String,
    public_base_url: String,
    runtime_transport: &'static str,
    storage: &'static str,
    master_key_version: i32,
    dogfood_exposure: bool,
    approval_webhook: bool,
}

#[derive(Debug, Serialize)]
struct DogfoodExposureEvent<'a> {
    event: &'static str,
    exposure_id: Uuid,
    public_url: &'a str,
    target_port: u16,
    created: bool,
}

#[derive(Debug, Serialize)]
struct DogfoodApprovalEvent {
    event: &'static str,
    approval_id: Uuid,
    execution_id: Uuid,
    revision: String,
}

#[derive(Debug, Serialize)]
struct DogfoodWebhookEvent {
    event: &'static str,
    approval_id: Uuid,
    notification_id: Uuid,
    revision: String,
}

pub async fn run_hosted_server(config: HostedServerConfig) -> Result<(), String> {
    let listener = TcpListener::bind(&config.bind)
        .await
        .map_err(|error| format!("bind hosted relay {}: {error}", config.bind))?;
    let local_port = listener
        .local_addr()
        .map_err(|error| format!("read hosted relay local address: {error}"))?
        .port();
    let local_runtime_base_url = format!("ws://127.0.0.1:{local_port}");

    let (capabilities, exposures, anonymous, identities, secrets, approvals, executions, storage) =
        open_hosted_stores(&config).await?;
    let bootstrap_workspace_id = if let Some(slug) = config.bootstrap_workspace.as_deref() {
        Some(
            ensure_operator_bootstrap_async(&identities, &secrets, slug, &config.public_base_url)
                .await?,
        )
    } else {
        None
    };
    let dogfood = dogfood_exposure_config(|key| std::env::var(key).ok(), local_port)?;
    if dogfood.is_some() && bootstrap_workspace_id.is_none() {
        return Err("ORTYO_DOGFOOD_EXPOSURE_PORT requires ORTYO_BOOTSTRAP_WORKSPACE".to_owned());
    }
    if config.approval_webhook_url.is_some() && bootstrap_workspace_id.is_none() {
        return Err("ORTYO_APPROVAL_WEBHOOK_URL requires ORTYO_BOOTSTRAP_WORKSPACE".to_owned());
    }
    let approval_webhook = match (
        bootstrap_workspace_id,
        config.approval_webhook_url.as_deref(),
    ) {
        (Some(workspace_id), Some(url)) => {
            ensure_webhook_secret(&secrets, workspace_id, url).await?;
            true
        }
        _ => false,
    };

    let anonymous_cleanup = anonymous.clone();

    let state = HostedRelayState::websocket_only_with_stores(
        RelayBroker::default(),
        capabilities,
        exposures,
        identities,
        secrets,
        config.public_base_url.clone(),
        &config.control_token,
    )
    .with_anonymous_store(anonymous)
    .with_approval_store(approvals)
    .with_execution_store(executions);

    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(60 * 60)).await;
            if let Err(error) = anonymous_cleanup.purge_expired_async().await {
                eprintln!(
                    "{}",
                    json!({
                        "event": "anonymous_expiry_purge_failed",
                        "error": format!("{error:?}")
                    })
                );
            }
        }
    });

    if approval_webhook {
        let webhook_state = state.clone();
        let workspace_id = bootstrap_workspace_id.expect("webhook requires bootstrap workspace");
        tokio::spawn(async move {
            run_worker(webhook_state, workspace_id).await;
        });
    }

    if let (Some(workspace_id), Some(dogfood)) = (bootstrap_workspace_id, dogfood.clone()) {
        let dogfood_state = state.clone();
        tokio::spawn(async move {
            if let Err(error) = run_dogfood_exposure(
                &dogfood_state,
                workspace_id,
                dogfood,
                local_runtime_base_url,
            )
            .await
            {
                eprintln!(
                    "{}",
                    json!({
                        "event": "dogfood_data_plane_failed",
                        "error": error
                    })
                );
                return;
            }

            if let Err(error) = wait_for_public_revision(&dogfood_state).await {
                eprintln!(
                    "{}",
                    json!({
                        "event": "dogfood_control_plane_failed",
                        "error": error
                    })
                );
                return;
            }

            if approval_webhook
                && let Err(error) = run_dogfood_approval_webhook(&dogfood_state, workspace_id).await
            {
                eprintln!(
                    "{}",
                    json!({
                        "event": "dogfood_approval_webhook_failed",
                        "error": error
                    })
                );
            }

            if let Err(error) = run_dogfood_approval_gate(&dogfood_state, workspace_id).await {
                eprintln!(
                    "{}",
                    json!({
                        "event": "dogfood_control_plane_failed",
                        "error": error
                    })
                );
            }
        });
    }

    let startup = HostedStartup {
        service: "hosted_relay",
        bind: config.bind,
        public_base_url: config.public_base_url,
        runtime_transport: "websocket",
        storage,
        master_key_version: config.secrets_keyring.active_version(),
        dogfood_exposure: dogfood.is_some(),
        approval_webhook,
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
        AnonymousExposureStore,
        HostedIdentityStore,
        SecretStore,
        ApprovalStore,
        ExecutionStore,
        &'static str,
    ),
    String,
> {
    if let Some(database_url) = &config.database_url {
        let database_url = database_url.clone();
        let secrets_keyring = config.secrets_keyring.clone();
        tokio::task::spawn_blocking(move || {
            let capabilities = CapabilityStore::open_postgres(&database_url)
                .map_err(|error| format!("open Postgres capability store: {error:?}"))?;
            let exposures = HostedExposureStore::open_postgres(&database_url)
                .map_err(|error| format!("open Postgres exposure store: {error:?}"))?;
            let anonymous = AnonymousExposureStore::open_postgres(&database_url)
                .map_err(|error| format!("open Postgres anonymous exposure store: {error:?}"))?;
            let identities = HostedIdentityStore::open_postgres(&database_url)
                .map_err(|error| format!("open Postgres identity store: {error:?}"))?;
            let secrets =
                SecretStore::open_postgres_with_keyring(&database_url, secrets_keyring.clone())
                    .map_err(|error| format!("open Postgres secret store: {error:?}"))?;
            let approvals = ApprovalStore::open_postgres_with_digest_keyring(
                &database_url,
                derive_digest_keyring(&secrets_keyring),
            )
            .map_err(|error| format!("open Postgres approval store: {error:?}"))?;
            let executions = ExecutionStore::open_postgres(&database_url)
                .map_err(|error| format!("open Postgres execution store: {error:?}"))?;
            Ok((
                capabilities,
                exposures,
                anonymous,
                identities,
                secrets,
                approvals,
                executions,
                "postgres",
            ))
        })
        .await
        .map_err(|error| format!("join Postgres store initialization: {error}"))?
    } else {
        let db_path = config.db_path.clone();
        let secrets_keyring = config.secrets_keyring.clone();
        tokio::task::spawn_blocking(move || {
            let capabilities = CapabilityStore::open(&db_path)
                .map_err(|error| format!("open SQLite capability store: {error:?}"))?;
            let exposures = HostedExposureStore::open(&db_path)
                .map_err(|error| format!("open SQLite exposure store: {error:?}"))?;
            let anonymous = AnonymousExposureStore::open(&db_path)
                .map_err(|error| format!("open SQLite anonymous exposure store: {error:?}"))?;
            let identities = HostedIdentityStore::open(&db_path)
                .map_err(|error| format!("open SQLite identity store: {error:?}"))?;
            let secrets = SecretStore::open_with_keyring(&db_path, secrets_keyring.clone())
                .map_err(|error| format!("open SQLite secret store: {error:?}"))?;
            let approvals = ApprovalStore::open_with_digest_keyring(
                &db_path,
                derive_digest_keyring(&secrets_keyring),
            )
            .map_err(|error| format!("open SQLite approval store: {error:?}"))?;
            let executions = ExecutionStore::open(&db_path)
                .map_err(|error| format!("open SQLite execution store: {error:?}"))?;
            Ok((
                capabilities,
                exposures,
                anonymous,
                identities,
                secrets,
                approvals,
                executions,
                "sqlite",
            ))
        })
        .await
        .map_err(|error| format!("join SQLite store initialization: {error}"))?
    }
}

async fn ensure_operator_bootstrap_async(
    identities: &HostedIdentityStore,
    secrets: &SecretStore,
    slug: &str,
    allowed_origin: &str,
) -> Result<Uuid, String> {
    let identities = identities.clone();
    let secrets = secrets.clone();
    let slug = slug.to_owned();
    let allowed_origin = allowed_origin.to_owned();
    tokio::task::spawn_blocking(move || {
        ensure_operator_bootstrap(&identities, &secrets, &slug, &allowed_origin)?;
        identities
            .find_workspace_by_slug(&slug)
            .map_err(|error| format!("find bootstrapped workspace: {error:?}"))?
            .map(|workspace| workspace.id)
            .ok_or_else(|| "bootstrapped workspace disappeared".to_owned())
    })
    .await
    .map_err(|error| format!("join operator bootstrap: {error}"))?
}

fn ensure_operator_bootstrap(
    identities: &HostedIdentityStore,
    secrets: &SecretStore,
    slug: &str,
    allowed_origin: &str,
) -> Result<(), String> {
    const SECRET_NAME: &str = "default-api-token";
    if let Some(workspace) = identities
        .find_workspace_by_slug(slug)
        .map_err(|error| format!("find bootstrap workspace: {error:?}"))?
    {
        if secrets.get_ref(workspace.id, SECRET_NAME).is_some() {
            secrets
                .bind_origin(workspace.id, SECRET_NAME, allowed_origin)
                .map_err(|error| format!("bind bootstrap credential origin: {error:?}"))?;
            let token = secrets
                .resolve(workspace.id, SECRET_NAME)
                .map_err(|error| {
                    format!("resolve bootstrap credential for scope upgrade: {error:?}")
                })?;
            identities
                .ensure_scope(&token, crate::hosted_identity::ApiScope::RequestsApprove)
                .map_err(|error| format!("upgrade bootstrap credential scopes: {error:?}"))?;
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
                    crate::hosted_identity::ApiScope::RequestsApprove,
                ],
            )
            .map_err(|error| format!("issue bootstrap recovery credential: {error:?}"))?;
        secrets
            .put_bound(workspace.id, SECRET_NAME, credential.token, allowed_origin)
            .map_err(|error| format!("persist bootstrap recovery credential: {error:?}"))?;
        return Ok(());
    }

    match identities.bootstrap_first_workspace(slug, "operator-bootstrap") {
        Ok((workspace, credential)) => {
            secrets
                .put_bound(workspace.id, SECRET_NAME, credential.token, allowed_origin)
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

fn dogfood_exposure_config(
    mut lookup: impl FnMut(&str) -> Option<String>,
    self_port: u16,
) -> Result<Option<DogfoodExposureConfig>, String> {
    let Some(port) = lookup("ORTYO_DOGFOOD_EXPOSURE_PORT").filter(|value| !value.trim().is_empty())
    else {
        return Ok(None);
    };
    let target_port = if port == "self" {
        self_port
    } else {
        parse_port(&port).map_err(|_| format!("invalid ORTYO_DOGFOOD_EXPOSURE_PORT: {port}"))?
    };
    let name = lookup("ORTYO_DOGFOOD_EXPOSURE_NAME")
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "ortyo-dogfood".to_owned());
    Ok(Some(DogfoodExposureConfig { name, target_port }))
}

async fn run_dogfood_exposure(
    state: &HostedRelayState,
    workspace_id: Uuid,
    config: DogfoodExposureConfig,
    local_runtime_base_url: String,
) -> Result<(), String> {
    let (exposure, created) = provision_dogfood_exposure(state, workspace_id, &config).await?;
    attach_dogfood_runtime(state, workspace_id, &exposure, &local_runtime_base_url).await?;
    verify_dogfood_data_plane(&exposure).await?;
    log_dogfood_exposure(&exposure, created, "dogfood_data_plane_ready");
    Ok(())
}

async fn run_dogfood_approval_webhook(
    state: &HostedRelayState,
    workspace_id: Uuid,
) -> Result<(), String> {
    const ATTEMPTS: usize = 30;
    const DOGFOOD_HEADER: &str = "x-ortyo-dogfood-secret";

    let canary_id = Uuid::now_v7();
    let query_canary = format!("ortyo-query-canary-{canary_id}");
    let header_canary = format!("ortyo-header-canary-{canary_id}");
    let body_canary = format!("ortyo-body-canary-{canary_id}");
    let request = HttpExecutionRequest {
        method: "POST".to_owned(),
        url: format!(
            "{}/llms.txt?dogfood_secret={query_canary}",
            state.public_base_url
        ),
        headers: BTreeMap::from([(DOGFOOD_HEADER.to_owned(), header_canary.clone())]),
        body: Some(json!({"secret": body_canary.clone()})),
        secret_headers: BTreeMap::new(),
        capture: vec![],
        timeout_ms: 5_000,
    };

    let created = execute_dogfood_control_request(
        state,
        workspace_id,
        "POST",
        "/_ortyo/hosted/approvals",
        Some(
            serde_json::to_value(&request)
                .map_err(|error| format!("serialize dogfood webhook approval: {error}"))?,
        ),
    )
    .await?;
    if created.status != 201 {
        return Err(format!(
            "dogfood webhook approval ask returned HTTP {}: {}",
            created.status, created.body
        ));
    }
    let approval: ApprovalRecord = serde_json::from_value(created.body)
        .map_err(|error| format!("parse dogfood webhook approval: {error}"))?;
    if approval.state != ApprovalState::Pending {
        return Err(format!(
            "dogfood webhook approval was not pending: {:?}",
            approval.state
        ));
    }
    if approval.summary.method != "POST"
        || approval.summary.origin != state.public_base_url
        || approval.summary.path != "/llms.txt"
        || approval.summary.header_names != [DOGFOOD_HEADER.to_owned()]
        || !approval.summary.secret_header_names.is_empty()
        || !approval
            .summary
            .body_fingerprint
            .as_deref()
            .is_some_and(|value| value.starts_with(KEYED_FINGERPRINT_PREFIX))
        || approval.summary.body_sha256.is_some()
        || !approval.summary.capture_names.is_empty()
    {
        return Err(
            "dogfood webhook approval summary was not the expected redacted action".to_owned(),
        );
    }

    let approval_json = serde_json::to_string(&approval)
        .map_err(|error| format!("serialize dogfood webhook approval record: {error}"))?;
    for canary in [&query_canary, &header_canary, &body_canary] {
        if approval_json.contains(canary) {
            return Err("dogfood webhook approval record leaked a request canary".to_owned());
        }
    }

    let notification = state
        .approvals
        .get_notification_for_approval_async(workspace_id, approval.approval_id)
        .await
        .map_err(|error| format!("load dogfood webhook notification: {error:?}"))?
        .ok_or_else(|| "dogfood webhook approval had no notification intent".to_owned())?;

    let payload = webhook_payload(notification.notification_id, &approval);
    let payload_json = serde_json::to_string(&payload)
        .map_err(|error| format!("serialize dogfood webhook payload proof: {error}"))?;
    for canary in [&query_canary, &header_canary, &body_canary] {
        if payload_json.contains(canary) {
            return Err("dogfood webhook payload leaked a request canary".to_owned());
        }
    }
    if payload.notification_id != notification.notification_id
        || payload.approval_id != approval.approval_id
        || payload.requested_at_unix_ms != approval.requested_at_unix_ms
        || payload.summary != approval.summary
    {
        return Err("dogfood webhook payload did not match the redacted approval".to_owned());
    }

    let headers = webhook_headers(notification.notification_id);
    let expected_idempotency_key = notification.notification_id.to_string();
    if headers.get("idempotency-key").map(String::as_str) != Some(expected_idempotency_key.as_str())
        || headers.get("x-ortyo-event").map(String::as_str) != Some("approval_requested")
    {
        return Err("dogfood webhook delivery headers were not deterministic".to_owned());
    }

    let mut delivered = false;
    for attempt in 0..ATTEMPTS {
        let current = state
            .approvals
            .get_notification_async(workspace_id, notification.notification_id)
            .await
            .map_err(|error| format!("poll dogfood webhook notification: {error:?}"))?
            .ok_or_else(|| "dogfood webhook notification disappeared".to_owned())?;
        if current.delivered_at_unix_ms.is_some() {
            delivered = true;
            break;
        }
        if attempt + 1 < ATTEMPTS {
            sleep(Duration::from_secs(1)).await;
        }
    }
    if !delivered {
        return Err("dogfood approval webhook was not delivered before timeout".to_owned());
    }

    let decision = execute_dogfood_control_request(
        state,
        workspace_id,
        "POST",
        &format!("/_ortyo/hosted/approvals/{}/decision", approval.approval_id),
        Some(json!({"decision": "deny"})),
    )
    .await?;
    if decision.status != 200 {
        return Err(format!(
            "dogfood webhook cleanup decision returned HTTP {}: {}",
            decision.status, decision.body
        ));
    }
    let denied: ApprovalRecord = serde_json::from_value(decision.body)
        .map_err(|error| format!("parse dogfood webhook denied approval: {error}"))?;
    if denied.state != ApprovalState::Denied {
        return Err(format!(
            "dogfood webhook cleanup was not denied: {:?}",
            denied.state
        ));
    }

    let revision = render_revision().unwrap_or_else(|| "unknown".to_owned());
    println!(
        "{}",
        serde_json::to_string(&DogfoodWebhookEvent {
            event: "dogfood_approval_webhook_redaction_ready",
            approval_id: approval.approval_id,
            notification_id: notification.notification_id,
            revision: revision.clone(),
        })
        .expect("dogfood webhook redaction event is serializable")
    );
    println!(
        "{}",
        serde_json::to_string(&DogfoodWebhookEvent {
            event: "dogfood_approval_webhook_ready",
            approval_id: approval.approval_id,
            notification_id: notification.notification_id,
            revision,
        })
        .expect("dogfood webhook event is serializable")
    );

    Ok(())
}

async fn run_dogfood_approval_gate(
    state: &HostedRelayState,
    workspace_id: Uuid,
) -> Result<(), String> {
    const ATTEMPTS: usize = 20;
    let inner_request = HttpExecutionRequest {
        method: "GET".to_owned(),
        url: format!("{}/healthz", state.public_base_url),
        headers: BTreeMap::new(),
        body: None,
        secret_headers: BTreeMap::new(),
        capture: vec![],
        timeout_ms: 5_000,
    };

    let approval = {
        let mut approval = None;
        for attempt in 0..ATTEMPTS {
            let evidence = execute_dogfood_control_request(
                state,
                workspace_id,
                "POST",
                "/_ortyo/hosted/approvals",
                Some(
                    serde_json::to_value(&inner_request)
                        .map_err(|error| format!("serialize dogfood approval request: {error}"))?,
                ),
            )
            .await?;

            if evidence.status == 201 {
                approval = Some(
                    serde_json::from_value::<ApprovalRecord>(evidence.body)
                        .map_err(|error| format!("parse dogfood approval response: {error}"))?,
                );
                break;
            }

            if attempt + 1 == ATTEMPTS {
                return Err(format!(
                    "dogfood approval ask returned HTTP {}: {}",
                    evidence.status, evidence.body
                ));
            }
            sleep(Duration::from_millis(500)).await;
        }
        approval.ok_or_else(|| "dogfood approval ask exhausted attempts".to_owned())?
    };

    if approval.state != ApprovalState::Pending {
        return Err(format!(
            "dogfood approval was not pending: {:?}",
            approval.state
        ));
    }
    if approval.summary.method != "GET"
        || approval.summary.path != "/healthz"
        || approval.summary.origin != state.public_base_url
        || approval.summary.body_fingerprint.is_some()
        || approval.summary.body_sha256.is_some()
        || !approval.summary.header_names.is_empty()
        || !approval.summary.secret_header_names.is_empty()
    {
        return Err(
            "dogfood approval summary was not the expected redacted health action".to_owned(),
        );
    }

    let notification = state
        .approvals
        .get_notification_for_approval_async(workspace_id, approval.approval_id)
        .await
        .map_err(|error| format!("load dogfood approval outbox intent: {error:?}"))?
        .ok_or_else(|| "dogfood approval was missing transactional outbox intent".to_owned())?;
    if notification.workspace_id != workspace_id
        || notification.approval_id != approval.approval_id
        || notification.event != ApprovalNotificationEvent::ApprovalRequested
        || notification.created_at_unix_ms != approval.requested_at_unix_ms
    {
        return Err("dogfood approval outbox intent did not match approval".to_owned());
    }

    let pending = dogfood_approval_inbox(state, workspace_id).await?;
    if !pending
        .iter()
        .any(|candidate| candidate.approval_id == approval.approval_id)
    {
        return Err("dogfood pending approval was missing from approval inbox".to_owned());
    }

    let decision = execute_dogfood_control_request(
        state,
        workspace_id,
        "POST",
        &format!("/_ortyo/hosted/approvals/{}/decision", approval.approval_id),
        Some(json!({"decision": "approve"})),
    )
    .await?;
    if decision.status != 200 {
        return Err(format!(
            "dogfood approval decision returned HTTP {}: {}",
            decision.status, decision.body
        ));
    }
    let approved: ApprovalRecord = serde_json::from_value(decision.body)
        .map_err(|error| format!("parse dogfood approved record: {error}"))?;
    if approved.state != ApprovalState::Approved {
        return Err(format!(
            "dogfood approval was not approved: {:?}",
            approved.state
        ));
    }

    let after_decision = dogfood_approval_inbox(state, workspace_id).await?;
    if after_decision
        .iter()
        .any(|candidate| candidate.approval_id == approval.approval_id)
    {
        return Err("dogfood decided approval remained in approval inbox".to_owned());
    }

    let mut mismatched = inner_request.clone();
    mismatched.url = format!("{}/llms.txt", state.public_base_url);
    let mismatch = execute_dogfood_control_request(
        state,
        workspace_id,
        "POST",
        &format!("/_ortyo/hosted/approvals/{}/execute", approval.approval_id),
        Some(
            serde_json::to_value(mismatched)
                .map_err(|error| format!("serialize dogfood mismatch request: {error}"))?,
        ),
    )
    .await?;
    if mismatch.status != 409 || mismatch.body["error"]["code"] != "approval_request_mismatch" {
        return Err(format!(
            "dogfood approval mismatch did not fail closed: HTTP {} {}",
            mismatch.status, mismatch.body
        ));
    }

    let act = execute_dogfood_control_request(
        state,
        workspace_id,
        "POST",
        &format!("/_ortyo/hosted/approvals/{}/execute", approval.approval_id),
        Some(
            serde_json::to_value(&inner_request)
                .map_err(|error| format!("serialize dogfood approved request: {error}"))?,
        ),
    )
    .await?;
    if act.status != 200 {
        return Err(format!(
            "dogfood approved execution returned HTTP {}: {}",
            act.status, act.body
        ));
    }
    let proof: ApprovedExecution = serde_json::from_value(act.body)
        .map_err(|error| format!("parse dogfood approved execution: {error}"))?;
    if proof.approval.state != ApprovalState::Consumed {
        return Err("dogfood approval was not consumed".to_owned());
    }
    if proof.approval.execution_id != Some(proof.execution.execution_id) {
        return Err("dogfood approval and execution identities differ".to_owned());
    }
    let expected_revision = render_revision();
    let verified_revision = match &proof.execution.outcome {
        ExecutionOutcome::Succeeded { evidence }
            if evidence.status == 200
                && health_body_matches(&evidence.body, expected_revision.as_deref()) =>
        {
            evidence.body["revision"]
                .as_str()
                .unwrap_or("unknown")
                .to_owned()
        }
        outcome => {
            return Err(format!(
                "dogfood approved health execution did not succeed: {outcome:?}"
            ));
        }
    };

    let durable = execute_dogfood_control_request(
        state,
        workspace_id,
        "GET",
        &format!("/_ortyo/hosted/executions/{}", proof.execution.execution_id),
        None,
    )
    .await?;
    if durable.status != 200 {
        return Err(format!(
            "dogfood durable execution query returned HTTP {}: {}",
            durable.status, durable.body
        ));
    }
    let durable: crate::execution_store::DurableExecutionRecord =
        serde_json::from_value(durable.body)
            .map_err(|error| format!("parse dogfood durable execution: {error}"))?;
    if durable.state != DurableExecutionState::Completed
        || durable.execution_id != proof.execution.execution_id
        || durable.workspace_id != workspace_id
        || durable.terminal_record() != Some(proof.execution.clone())
    {
        return Err("dogfood durable execution did not match immediate proof".to_owned());
    }

    let replay = execute_dogfood_control_request(
        state,
        workspace_id,
        "POST",
        &format!("/_ortyo/hosted/approvals/{}/execute", approval.approval_id),
        Some(
            serde_json::to_value(&inner_request)
                .map_err(|error| format!("serialize dogfood replay request: {error}"))?,
        ),
    )
    .await?;
    if replay.status != 409 || replay.body["error"]["code"] != "approval_consumed" {
        return Err(format!(
            "dogfood consumed approval replay did not fail closed: HTTP {} {}",
            replay.status, replay.body
        ));
    }

    println!(
        "{}",
        serde_json::to_string(&DogfoodApprovalEvent {
            event: "dogfood_control_plane_ready",
            approval_id: approval.approval_id,
            execution_id: proof.execution.execution_id,
            revision: verified_revision,
        })
        .expect("dogfood approval event is serializable")
    );

    Ok(())
}

async fn dogfood_approval_inbox(
    state: &HostedRelayState,
    workspace_id: Uuid,
) -> Result<Vec<ApprovalRecord>, String> {
    let inbox = execute_dogfood_control_request(
        state,
        workspace_id,
        "GET",
        "/_ortyo/hosted/approvals",
        None,
    )
    .await?;
    if inbox.status != 200 {
        return Err(format!(
            "dogfood approval inbox returned HTTP {}: {}",
            inbox.status, inbox.body
        ));
    }
    serde_json::from_value(inbox.body)
        .map_err(|error| format!("parse dogfood approval inbox: {error}"))
}

async fn execute_dogfood_control_request(
    state: &HostedRelayState,
    workspace_id: Uuid,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> Result<crate::execution::ExecutionEvidence, String> {
    let mut secret_headers = BTreeMap::new();
    secret_headers.insert(
        "authorization".to_owned(),
        SecretHeaderBinding::SecretRef {
            secret_ref: "ortyo://secrets/default-api-token".to_owned(),
            prefix: "Bearer ".to_owned(),
            suffix: String::new(),
        },
    );

    state
        .executor
        .execute(
            workspace_id,
            HttpExecutionRequest {
                method: method.to_owned(),
                url: format!("{}{}", state.public_base_url, path),
                headers: BTreeMap::new(),
                body,
                secret_headers,
                capture: vec![],
                timeout_ms: 5_000,
            },
        )
        .await
        .map_err(|error| format!("dogfood control request failed: {error:?}"))
}

async fn wait_for_public_revision(state: &HostedRelayState) -> Result<(), String> {
    const ATTEMPTS: usize = 60;
    let Some(expected) = render_revision() else {
        return Ok(());
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|error| format!("build public revision client: {error}"))?;

    for attempt in 0..ATTEMPTS {
        if let Ok(response) = client
            .get(format!("{}/healthz", state.public_base_url))
            .send()
            .await
            && response.status().is_success()
            && let Ok(body) = response.json::<serde_json::Value>().await
            && body["revision"].as_str() == Some(expected.as_str())
        {
            return Ok(());
        }
        if attempt + 1 < ATTEMPTS {
            sleep(Duration::from_secs(1)).await;
        }
    }

    Err(format!(
        "public revision did not converge to deployment {expected}"
    ))
}

async fn provision_dogfood_exposure(
    state: &HostedRelayState,
    workspace_id: Uuid,
    config: &DogfoodExposureConfig,
) -> Result<(HostedExposureRecord, bool), String> {
    const ATTEMPTS: usize = 20;
    const API_TOKEN_REF: &str = "ortyo://secrets/default-api-token";
    const RUNTIME_CAPABILITY_SECRET: &str = "dogfood-runtime-capability";

    for attempt in 0..ATTEMPTS {
        if let Some(exposure) = reusable_dogfood_exposure(state, workspace_id, config).await? {
            return Ok((exposure, false));
        }

        let mut secret_headers = BTreeMap::new();
        secret_headers.insert(
            "authorization".to_owned(),
            SecretHeaderBinding::SecretRef {
                secret_ref: API_TOKEN_REF.to_owned(),
                prefix: "Bearer ".to_owned(),
                suffix: String::new(),
            },
        );
        let request = HttpExecutionRequest {
            method: "POST".to_owned(),
            url: format!("{}/_ortyo/hosted/exposures", state.public_base_url),
            headers: BTreeMap::new(),
            body: Some(json!({
                "name": config.name.clone(),
                "target_port": config.target_port
            })),
            secret_headers,
            capture: vec![SecretCapture {
                json_pointer: "/runtime_capability".to_owned(),
                secret_name: RUNTIME_CAPABILITY_SECRET.to_owned(),
            }],
            timeout_ms: 5_000,
        };

        match state.executor.execute(workspace_id, request).await {
            Ok(evidence) if evidence.status == 201 => {
                if evidence.body["runtime_capability"] != "[REDACTED]" {
                    return Err("dogfood runtime capability was not redacted".to_owned());
                }
                let exposure_id = evidence.body["exposure_id"]
                    .as_str()
                    .ok_or_else(|| "dogfood response missing exposure_id".to_owned())?
                    .parse::<Uuid>()
                    .map_err(|_| "dogfood response has invalid exposure_id".to_owned())?;
                let public_url = evidence.body["public_url"]
                    .as_str()
                    .ok_or_else(|| "dogfood response missing public_url".to_owned())?
                    .to_owned();
                let captured = evidence.captured_secrets.iter().any(|secret| {
                    secret.secret_ref == format!("ortyo://secrets/{RUNTIME_CAPABILITY_SECRET}")
                });
                if !captured {
                    return Err("dogfood runtime capability was not captured".to_owned());
                }
                let exposure = state
                    .exposures
                    .get_async(exposure_id)
                    .await
                    .map_err(|error| format!("load dogfood exposure: {error:?}"))?
                    .ok_or_else(|| "dogfood exposure was not persisted".to_owned())?;
                if exposure.public_url != public_url {
                    return Err("dogfood persisted public URL does not match response".to_owned());
                }
                return Ok((exposure, true));
            }
            Ok(evidence) => {
                if attempt + 1 == ATTEMPTS {
                    return Err(format!(
                        "dogfood exposure returned HTTP {}",
                        evidence.status
                    ));
                }
            }
            Err(ExecutionError::RequestFailed) => {
                if attempt + 1 == ATTEMPTS {
                    return Err("dogfood exposure request failed".to_owned());
                }
            }
            Err(error) => return Err(format!("dogfood exposure execution failed: {error:?}")),
        }

        sleep(Duration::from_millis(500)).await;
    }

    Err("dogfood exposure attempts exhausted".to_owned())
}

async fn reusable_dogfood_exposure(
    state: &HostedRelayState,
    workspace_id: Uuid,
    config: &DogfoodExposureConfig,
) -> Result<Option<HostedExposureRecord>, String> {
    const RUNTIME_CAPABILITY_SECRET: &str = "dogfood-runtime-capability";

    let mut exposures = state
        .exposures
        .list_for_workspace_async(workspace_id)
        .await
        .map_err(|error| format!("list dogfood exposures: {error:?}"))?;
    exposures.reverse();

    for mut exposure in exposures
        .into_iter()
        .filter(|exposure| !exposure.revoked && exposure.name == config.name)
    {
        if exposure.target_port != config.target_port {
            state
                .capabilities
                .revoke_exposure_async(exposure.exposure_id)
                .await
                .map_err(|error| format!("revoke mismatched dogfood capability: {error:?}"))?;
            state
                .exposures
                .revoke_async(exposure.exposure_id)
                .await
                .map_err(|error| format!("revoke mismatched dogfood exposure: {error:?}"))?;
            continue;
        }

        let stored_capability = state
            .executor
            .secret_store()
            .resolve_async(workspace_id, RUNTIME_CAPABILITY_SECRET.to_owned())
            .await;
        if let Ok(capability) = stored_capability
            && state
                .capabilities
                .authorize_async(exposure.exposure_id, &capability)
                .await
                .is_ok()
        {
            return Ok(Some(exposure));
        }

        let rotated = state
            .capabilities
            .rotate_exposure_async(exposure.exposure_id, state.capability_ttl)
            .await
            .map_err(|error| format!("rotate dogfood capability: {error:?}"))?;
        let expires_at = rotated
            .expires_at
            .duration_since(UNIX_EPOCH)
            .map_err(|error| format!("dogfood capability expiry before unix epoch: {error}"))?
            .as_secs();
        state
            .executor
            .secret_store()
            .rotate_async(
                workspace_id,
                RUNTIME_CAPABILITY_SECRET.to_owned(),
                rotated.token,
            )
            .await
            .map_err(|error| format!("persist rotated dogfood capability: {error:?}"))?;
        state
            .exposures
            .update_capability_expiry_async(exposure.exposure_id, expires_at)
            .await
            .map_err(|error| format!("persist dogfood capability expiry: {error:?}"))?;
        exposure.capability_expires_at_unix_seconds = expires_at;
        return Ok(Some(exposure));
    }

    Ok(None)
}

async fn attach_dogfood_runtime(
    state: &HostedRelayState,
    workspace_id: Uuid,
    exposure: &HostedExposureRecord,
    local_runtime_base_url: &str,
) -> Result<(), String> {
    const RUNTIME_CAPABILITY_SECRET: &str = "dogfood-runtime-capability";

    let capability = state
        .executor
        .secret_store()
        .resolve_async(workspace_id, RUNTIME_CAPABILITY_SECRET.to_owned())
        .await
        .map_err(|_| "resolve dogfood runtime capability".to_owned())?;
    state
        .capabilities
        .authorize_async(exposure.exposure_id, &capability)
        .await
        .map_err(|_| "authorize dogfood runtime capability".to_owned())?;

    let runtime_url = format!(
        "{local_runtime_base_url}/_ortyo/runtime/{}",
        exposure.exposure_id
    );
    let provision = ProvisionedExposure {
        exposure_id: exposure.exposure_id,
        workspace_id: Some(workspace_id),
        name: exposure.name.clone(),
        public_url: exposure.public_url.clone(),
        relay_addr: None,
        runtime_url,
        runtime_capability: capability,
        capability_expires_at_unix_seconds: exposure.capability_expires_at_unix_seconds,
        target_port: exposure.target_port,
        mode: ExposureMode::Relay,
        access: ExposureAccess::Public,
    };
    let runtime_state = AppState::default();
    let manager = runtime_state.hosted_runtimes.clone();
    manager
        .attach(runtime_state, provision)
        .await
        .map_err(|_| "attach dogfood runtime".to_owned())?;
    Ok(())
}

async fn verify_dogfood_data_plane(exposure: &HostedExposureRecord) -> Result<(), String> {
    const ATTEMPTS: usize = 60;
    let url = format!("{}/healthz", exposure.public_url);
    let expected_revision = render_revision();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|_| "build dogfood verification client".to_owned())?;

    for attempt in 0..ATTEMPTS {
        if let Ok(response) = client.get(&url).send().await
            && response.status().is_success()
            && let Ok(body) = response.json::<serde_json::Value>().await
            && health_body_matches(&body, expected_revision.as_deref())
        {
            return Ok(());
        }
        if attempt + 1 < ATTEMPTS {
            sleep(Duration::from_secs(1)).await;
        }
    }

    Err("dogfood data-plane verification failed".to_owned())
}

fn render_revision() -> Option<String> {
    std::env::var("RENDER_GIT_COMMIT")
        .ok()
        .filter(|value| !value.trim().is_empty())
}

fn health_body_matches(body: &serde_json::Value, expected_revision: Option<&str>) -> bool {
    body["ok"] == true
        && body["service"] == "hosted_relay"
        && expected_revision.is_none_or(|revision| body["revision"].as_str() == Some(revision))
}

fn log_dogfood_exposure(exposure: &HostedExposureRecord, created: bool, event: &'static str) {
    println!(
        "{}",
        serde_json::to_string(&DogfoodExposureEvent {
            event,
            exposure_id: exposure.exposure_id,
            public_url: &exposure.public_url,
            target_port: exposure.target_port,
            created,
        })
        .expect("dogfood event is serializable")
    );
}

fn parse_positive_key_version(name: &str, value: &str) -> Result<i32, String> {
    let version: i32 = value
        .parse()
        .map_err(|_| format!("{name} must be a positive integer"))?;
    if version <= 0 {
        return Err(format!("{name} must be a positive integer"));
    }
    Ok(version)
}

fn parse_previous_secret_keys(value: &str) -> Result<Vec<(i32, [u8; 32])>, String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let (version, key) = entry.split_once(':').ok_or_else(|| {
                "ORTYO_SECRETS_PREVIOUS_KEYS must contain version:64hex entries".to_owned()
            })?;
            let version =
                parse_positive_key_version("ORTYO_SECRETS_PREVIOUS_KEYS version", version)?;
            let key = decode_master_key(key).map_err(|_| {
                "ORTYO_SECRETS_PREVIOUS_KEYS keys must be exactly 64 hexadecimal characters"
                    .to_owned()
            })?;
            Ok((version, key))
        })
        .collect()
}

fn keyring_config_error(error: VersionedKeyringError) -> String {
    match error {
        VersionedKeyringError::InvalidVersion(_) => {
            "secret key versions must be positive integers".to_owned()
        }
        VersionedKeyringError::DuplicateVersion(version) => {
            format!("duplicate secret key version: {version}")
        }
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
    use std::time::Duration;

    use super::{
        DogfoodExposureConfig, HostedServerConfig, dogfood_exposure_config,
        ensure_operator_bootstrap_async, health_body_matches, open_hosted_stores,
        reusable_dogfood_exposure,
    };
    use crate::{
        hosted::HostedRelayState,
        hosted_identity::{ApiScope, HostedIdentityStore},
        hosted_state::{HostedExposureRecord, HostedExposureStore},
        relay::RelayBroker,
        relay_auth::{CapabilityError, CapabilityStore},
        secret::SecretStore,
    };
    use uuid::Uuid;

    #[test]
    fn hosted_config_defaults_master_key_version_to_one() {
        let key = "11".repeat(32);
        let config = HostedServerConfig::from_lookup(|name| match name {
            "ORTYO_CONTROL_TOKEN" => Some("control".to_owned()),
            "ORTYO_SECRETS_KEY" => Some(key.clone()),
            _ => None,
        })
        .unwrap();

        assert_eq!(config.secrets_keyring.active_version(), 1);
        assert_eq!(config.secrets_keyring.active_key(), &[0x11; 32]);
        assert_eq!(config.secrets_keyring.versions().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn hosted_config_parses_active_and_previous_master_keys() {
        let active = "22".repeat(32);
        let previous = format!("1:{}", "11".repeat(32));
        let config = HostedServerConfig::from_lookup(|name| match name {
            "ORTYO_CONTROL_TOKEN" => Some("control".to_owned()),
            "ORTYO_SECRETS_KEY" => Some(active.clone()),
            "ORTYO_SECRETS_KEY_VERSION" => Some("2".to_owned()),
            "ORTYO_SECRETS_PREVIOUS_KEYS" => Some(previous.clone()),
            _ => None,
        })
        .unwrap();

        assert_eq!(config.secrets_keyring.active_version(), 2);
        assert_eq!(config.secrets_keyring.active_key(), &[0x22; 32]);
        assert_eq!(config.secrets_keyring.key(1), Some(&[0x11; 32]));
        assert_eq!(
            config.secrets_keyring.versions().collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn hosted_config_rejects_invalid_or_duplicate_master_key_versions() {
        let key = "33".repeat(32);
        let invalid = HostedServerConfig::from_lookup(|name| match name {
            "ORTYO_CONTROL_TOKEN" => Some("control".to_owned()),
            "ORTYO_SECRETS_KEY" => Some(key.clone()),
            "ORTYO_SECRETS_KEY_VERSION" => Some("0".to_owned()),
            _ => None,
        })
        .unwrap_err();
        assert_eq!(
            invalid,
            "ORTYO_SECRETS_KEY_VERSION must be a positive integer"
        );

        let duplicate = HostedServerConfig::from_lookup(|name| match name {
            "ORTYO_CONTROL_TOKEN" => Some("control".to_owned()),
            "ORTYO_SECRETS_KEY" => Some(key.clone()),
            "ORTYO_SECRETS_KEY_VERSION" => Some("2".to_owned()),
            "ORTYO_SECRETS_PREVIOUS_KEYS" => {
                Some(format!("1:{},1:{}", "11".repeat(32), "12".repeat(32)))
            }
            _ => None,
        })
        .unwrap_err();
        assert_eq!(duplicate, "duplicate secret key version: 1");

        let active_duplicate = HostedServerConfig::from_lookup(|name| match name {
            "ORTYO_CONTROL_TOKEN" => Some("control".to_owned()),
            "ORTYO_SECRETS_KEY" => Some(key.clone()),
            "ORTYO_SECRETS_KEY_VERSION" => Some("2".to_owned()),
            "ORTYO_SECRETS_PREVIOUS_KEYS" => Some(format!("2:{}", "11".repeat(32))),
            _ => None,
        })
        .unwrap_err();
        assert_eq!(active_duplicate, "duplicate secret key version: 2");
    }

    #[test]
    fn dogfood_health_requires_render_revision_when_present() {
        let body = serde_json::json!({
            "ok": true,
            "service": "hosted_relay",
            "revision": "abc123"
        });

        assert!(health_body_matches(&body, None));
        assert!(health_body_matches(&body, Some("abc123")));
        assert!(!health_body_matches(&body, Some("different")));
    }

    #[tokio::test]
    async fn operator_bootstrap_captures_token_and_is_idempotent() {
        let path = std::env::temp_dir().join(format!("ortyo-bootstrap-{}.db", Uuid::now_v7()));
        let identities = HostedIdentityStore::open(&path).unwrap();
        let secrets = SecretStore::open(&path, [41; 32]).unwrap();

        ensure_operator_bootstrap_async(&identities, &secrets, "serhii", "https://ortyo.example")
            .await
            .unwrap();
        let workspace = identities
            .find_workspace_by_slug("serhii")
            .unwrap()
            .unwrap();
        let token = secrets.resolve(workspace.id, "default-api-token").unwrap();
        identities
            .authorize(&token, ApiScope::RequestsExecute)
            .unwrap();
        identities
            .authorize(&token, ApiScope::RequestsApprove)
            .unwrap();

        let first_ref = secrets.get_ref(workspace.id, "default-api-token").unwrap();
        assert_eq!(
            first_ref.allowed_origin.as_deref(),
            Some("https://ortyo.example")
        );
        ensure_operator_bootstrap_async(&identities, &secrets, "serhii", "https://ortyo.example")
            .await
            .unwrap();
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
    async fn dogfood_rotation_preserves_exposure_identity_after_capability_expiry() {
        let workspace_id = Uuid::now_v7();
        let exposure_id = Uuid::now_v7();
        let capabilities = CapabilityStore::default();
        let exposures = HostedExposureStore::default();
        let secrets = SecretStore::default();

        let expired = capabilities.issue(exposure_id, Duration::ZERO).unwrap();
        secrets
            .put(
                workspace_id,
                "dogfood-runtime-capability",
                expired.token.clone(),
            )
            .unwrap();
        let original = HostedExposureRecord {
            exposure_id,
            workspace_id: Some(workspace_id),
            name: "ortyo-dogfood".to_owned(),
            target_port: 10000,
            public_url: format!("https://ortyo.example/e/{exposure_id}"),
            runtime_url: format!("wss://ortyo.example/_ortyo/runtime/{exposure_id}"),
            capability_expires_at_unix_seconds: 0,
            revoked: false,
        };
        exposures.save(&original).unwrap();

        let mut state = HostedRelayState::websocket_only_with_stores(
            RelayBroker::default(),
            capabilities.clone(),
            exposures.clone(),
            HostedIdentityStore::default(),
            secrets.clone(),
            "https://ortyo.example",
            "control-token",
        );
        state.capability_ttl = Duration::from_secs(60);

        let reused = reusable_dogfood_exposure(
            &state,
            workspace_id,
            &DogfoodExposureConfig {
                name: "ortyo-dogfood".to_owned(),
                target_port: 10000,
            },
        )
        .await
        .unwrap()
        .unwrap();

        assert_eq!(reused.exposure_id, original.exposure_id);
        assert_eq!(reused.public_url, original.public_url);
        assert!(!reused.revoked);
        assert!(reused.capability_expires_at_unix_seconds > 0);

        let rotated = secrets
            .resolve(workspace_id, "dogfood-runtime-capability")
            .unwrap();
        assert_ne!(rotated, expired.token);
        assert_eq!(
            capabilities.authorize(exposure_id, &expired.token),
            Err(CapabilityError::Revoked)
        );
        assert_eq!(capabilities.authorize(exposure_id, &rotated), Ok(()));

        let persisted = exposures.get(exposure_id).unwrap().unwrap();
        assert_eq!(persisted.exposure_id, original.exposure_id);
        assert_eq!(persisted.public_url, original.public_url);
        assert_eq!(
            persisted.capability_expires_at_unix_seconds,
            reused.capability_expires_at_unix_seconds
        );
    }

    #[test]
    fn dogfood_exposure_is_opt_in_and_has_a_stable_default_name() {
        assert_eq!(dogfood_exposure_config(|_| None, 4242).unwrap(), None);

        let config = dogfood_exposure_config(
            |key| match key {
                "ORTYO_DOGFOOD_EXPOSURE_PORT" => Some("3000".to_owned()),
                _ => None,
            },
            4242,
        )
        .unwrap();
        assert_eq!(
            config,
            Some(DogfoodExposureConfig {
                name: "ortyo-dogfood".to_owned(),
                target_port: 3000,
            })
        );
    }

    #[test]
    fn dogfood_exposure_can_target_the_hosted_service_itself() {
        let config = dogfood_exposure_config(
            |key| match key {
                "ORTYO_DOGFOOD_EXPOSURE_PORT" => Some("self".to_owned()),
                _ => None,
            },
            10000,
        )
        .unwrap()
        .unwrap();

        assert_eq!(config.target_port, 10000);
    }

    #[test]
    fn dogfood_exposure_rejects_invalid_port() {
        let error = dogfood_exposure_config(
            |key| match key {
                "ORTYO_DOGFOOD_EXPOSURE_PORT" => Some("0".to_owned()),
                _ => None,
            },
            4242,
        )
        .unwrap_err();
        assert_eq!(error, "invalid ORTYO_DOGFOOD_EXPOSURE_PORT: 0");
    }

    #[tokio::test]
    async fn postgres_initialization_fails_without_nested_runtime_panic() {
        let config = HostedServerConfig {
            bind: "127.0.0.1:0".to_owned(),
            public_base_url: "http://127.0.0.1".to_owned(),
            control_token: "test-control-token".to_owned(),
            database_url: Some("postgresql://127.0.0.1:1/ortyo".to_owned()),
            db_path: "unused.db".to_owned(),
            secrets_keyring: VersionedKeyring::single([7; 32]),
            bootstrap_workspace: None,
            approval_webhook_url: None,
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
