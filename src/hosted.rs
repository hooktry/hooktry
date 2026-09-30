use std::time::{Duration, UNIX_EPOCH};

use axum::{
    Json, Router,
    extract::{Path, State, ws::WebSocketUpgrade},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    domain::{ExposureAccess, ExposureMode},
    hosted_state::{HostedExposureRecord, HostedExposureStore},
    relay::RelayBroker,
    relay_auth::{CapabilityStore, token_digest},
    relay_ingress::{RelayIngressState, relay_ingress_app},
    websocket_transport::serve_websocket,
};

#[derive(Clone)]
pub struct HostedRelayState {
    pub broker: RelayBroker,
    pub capabilities: CapabilityStore,
    pub public_base_url: String,
    pub relay_addr: Option<String>,
    pub runtime_ws_base_url: String,
    pub capability_ttl: Duration,
    pub exposures: HostedExposureStore,
    control_token_digest: [u8; 32],
}

impl HostedRelayState {
    pub fn new(
        broker: RelayBroker,
        capabilities: CapabilityStore,
        public_base_url: impl Into<String>,
        relay_addr: impl Into<String>,
        control_token: &str,
    ) -> Self {
        let public_base_url = public_base_url.into().trim_end_matches('/').to_owned();
        let runtime_ws_base_url = websocket_base_url(&public_base_url);
        Self {
            broker,
            capabilities,
            public_base_url,
            relay_addr: Some(relay_addr.into()),
            runtime_ws_base_url,
            capability_ttl: Duration::from_secs(15 * 60),
            exposures: HostedExposureStore::default(),
            control_token_digest: token_digest(control_token),
        }
    }

    pub fn websocket_only(
        broker: RelayBroker,
        capabilities: CapabilityStore,
        public_base_url: impl Into<String>,
        control_token: &str,
    ) -> Self {
        Self::websocket_only_with_store(
            broker,
            capabilities,
            HostedExposureStore::default(),
            public_base_url,
            control_token,
        )
    }

    pub fn websocket_only_with_store(
        broker: RelayBroker,
        capabilities: CapabilityStore,
        exposures: HostedExposureStore,
        public_base_url: impl Into<String>,
        control_token: &str,
    ) -> Self {
        let public_base_url = public_base_url.into().trim_end_matches('/').to_owned();
        let runtime_ws_base_url = websocket_base_url(&public_base_url);
        Self {
            broker,
            capabilities,
            public_base_url,
            relay_addr: None,
            runtime_ws_base_url,
            capability_ttl: Duration::from_secs(15 * 60),
            exposures,
            control_token_digest: token_digest(control_token),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ProvisionExposureRequest {
    pub name: String,
    pub target_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvisionedExposure {
    pub exposure_id: Uuid,
    pub name: String,
    pub public_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay_addr: Option<String>,
    pub runtime_url: String,
    pub runtime_capability: String,
    pub capability_expires_at_unix_seconds: u64,
    pub target_port: u16,
    pub mode: ExposureMode,
    pub access: ExposureAccess,
}

pub fn hosted_relay_app(state: HostedRelayState) -> Router {
    let ingress = relay_ingress_app(RelayIngressState::new(state.broker.clone()));
    Router::new()
        .route("/_ortyo/health", get(health))
        .route("/healthz", get(health))
        .route("/_ortyo/hosted/exposures", post(provision_exposure))
        .route(
            "/_ortyo/runtime/{exposure_id}",
            get(runtime_websocket).delete(revoke_runtime),
        )
        .with_state(state)
        .merge(ingress)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ok": true,
        "service": "hosted_relay"
    }))
}

async fn provision_exposure(
    State(state): State<HostedRelayState>,
    headers: HeaderMap,
    Json(request): Json<ProvisionExposureRequest>,
) -> Result<(StatusCode, Json<ProvisionedExposure>), StatusCode> {
    authorize_control(&state, &headers)?;

    if request.name.trim().is_empty() || request.target_port == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let exposure_id = Uuid::now_v7();
    let capability = state
        .capabilities
        .issue(exposure_id, state.capability_ttl)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let expires_at = capability
        .expires_at
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .as_secs();

    let public_url = format!("{}/e/{exposure_id}", state.public_base_url);
    let runtime_url = format!("{}/_ortyo/runtime/{exposure_id}", state.runtime_ws_base_url);

    if state
        .exposures
        .save(&HostedExposureRecord {
            exposure_id,
            name: request.name.clone(),
            target_port: request.target_port,
            public_url: public_url.clone(),
            runtime_url: runtime_url.clone(),
            capability_expires_at_unix_seconds: expires_at,
            revoked: false,
        })
        .is_err()
    {
        let _ = state.capabilities.revoke(&capability.token);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok((
        StatusCode::CREATED,
        Json(ProvisionedExposure {
            exposure_id,
            name: request.name,
            public_url,
            relay_addr: state.relay_addr,
            runtime_url,
            runtime_capability: capability.token,
            capability_expires_at_unix_seconds: expires_at,
            target_port: request.target_port,
            mode: ExposureMode::Relay,
            access: ExposureAccess::Public,
        }),
    ))
}

fn authorize_control(state: &HostedRelayState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let token = bearer_token(headers).ok_or(StatusCode::UNAUTHORIZED)?;
    if token_digest(token) != state.control_token_digest {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(())
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
}

async fn revoke_runtime(
    State(state): State<HostedRelayState>,
    Path(exposure_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, StatusCode> {
    let capability = bearer_token(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    active_hosted_exposure(&state, exposure_id)?;
    state
        .capabilities
        .authorize(exposure_id, capability)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    state
        .capabilities
        .revoke(capability)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    state
        .exposures
        .revoke(exposure_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.broker.disconnect(exposure_id).await;
    Ok(StatusCode::NO_CONTENT)
}

async fn runtime_websocket(
    State(state): State<HostedRelayState>,
    Path(exposure_id): Path<Uuid>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    let capability = bearer_token(&headers).ok_or(StatusCode::UNAUTHORIZED)?;
    active_hosted_exposure(&state, exposure_id)?;

    state
        .capabilities
        .authorize(exposure_id, capability)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let broker = state.broker.clone();
    Ok(ws
        .max_message_size(2 * 1024 * 1024)
        .on_upgrade(move |socket| async move {
            let _ = serve_websocket(socket, broker, exposure_id).await;
        }))
}

fn active_hosted_exposure(
    state: &HostedRelayState,
    exposure_id: Uuid,
) -> Result<HostedExposureRecord, StatusCode> {
    let exposure = state
        .exposures
        .get(exposure_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    if exposure.revoked {
        return Err(StatusCode::GONE);
    }
    Ok(exposure)
}

fn websocket_base_url(public_base_url: &str) -> String {
    if let Some(rest) = public_base_url.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = public_base_url.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        public_base_url.to_owned()
    }
}
