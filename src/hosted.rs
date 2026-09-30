use std::time::{Duration, UNIX_EPOCH};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::post,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    domain::{ExposureAccess, ExposureMode},
    relay::RelayBroker,
    relay_auth::CapabilityStore,
    relay_ingress::{RelayIngressState, relay_ingress_app},
};

#[derive(Clone)]
pub struct HostedRelayState {
    pub broker: RelayBroker,
    pub capabilities: CapabilityStore,
    pub public_base_url: String,
    pub relay_addr: String,
    pub capability_ttl: Duration,
}

impl HostedRelayState {
    pub fn new(
        broker: RelayBroker,
        capabilities: CapabilityStore,
        public_base_url: impl Into<String>,
        relay_addr: impl Into<String>,
    ) -> Self {
        Self {
            broker,
            capabilities,
            public_base_url: public_base_url.into().trim_end_matches('/').to_owned(),
            relay_addr: relay_addr.into(),
            capability_ttl: Duration::from_secs(15 * 60),
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
    pub relay_addr: String,
    pub runtime_capability: String,
    pub capability_expires_at_unix_seconds: u64,
    pub target_port: u16,
    pub mode: ExposureMode,
    pub access: ExposureAccess,
}

pub fn hosted_relay_app(state: HostedRelayState) -> Router {
    let ingress = relay_ingress_app(RelayIngressState::new(state.broker.clone()));
    Router::new()
        .route("/_ortyo/hosted/exposures", post(provision_exposure))
        .merge(ingress)
        .with_state(state)
}

async fn provision_exposure(
    State(state): State<HostedRelayState>,
    Json(request): Json<ProvisionExposureRequest>,
) -> Result<(StatusCode, Json<ProvisionedExposure>), StatusCode> {
    if request.name.trim().is_empty() || request.target_port == 0 {
        return Err(StatusCode::BAD_REQUEST);
    }

    let exposure_id = Uuid::now_v7();
    let capability = state.capabilities.issue(exposure_id, state.capability_ttl);
    let expires_at = capability
        .expires_at
        .duration_since(UNIX_EPOCH)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .as_secs();

    Ok((
        StatusCode::CREATED,
        Json(ProvisionedExposure {
            exposure_id,
            name: request.name,
            public_url: format!("{}/e/{exposure_id}", state.public_base_url),
            relay_addr: state.relay_addr,
            runtime_capability: capability.token,
            capability_expires_at_unix_seconds: expires_at,
            target_port: request.target_port,
            mode: ExposureMode::Relay,
            access: ExposureAccess::Public,
        }),
    ))
}
