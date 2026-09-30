use std::time::{Duration, UNIX_EPOCH};

use axum::{
    Json, Router,
    extract::{Path, State, ws::WebSocketUpgrade},
    http::{HeaderMap, StatusCode, header::AUTHORIZATION},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    domain::{ExposureAccess, ExposureMode},
    hosted_identity::{
        ApiAuthorization, ApiScope, HostedIdentityStore, IdentityError, IssuedApiCredential,
        Workspace,
    },
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
    pub identities: HostedIdentityStore,
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
            identities: HostedIdentityStore::default(),
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
        Self::websocket_only_with_stores(
            broker,
            capabilities,
            exposures,
            HostedIdentityStore::default(),
            public_base_url,
            control_token,
        )
    }

    pub fn websocket_only_with_stores(
        broker: RelayBroker,
        capabilities: CapabilityStore,
        exposures: HostedExposureStore,
        identities: HostedIdentityStore,
        public_base_url: impl Into<String>,
        control_token: &str,
    ) -> Self {
        let public_base_url = public_base_url.into().trim_end_matches('/').to_owned();
        let runtime_ws_base_url = websocket_base_url(&public_base_url);
        Self {
            broker,
            capabilities,
            identities,
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

#[derive(Debug, Deserialize)]
struct CreateWorkspaceRequest {
    slug: String,
}

#[derive(Debug, Deserialize)]
struct IssueCredentialRequest {
    name: String,
    scopes: Vec<ApiScope>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvisionedExposure {
    pub exposure_id: Uuid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<Uuid>,
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

#[derive(Debug)]
struct HostedApiError {
    status: StatusCode,
    code: &'static str,
}

impl HostedApiError {
    fn new(status: StatusCode, code: &'static str) -> Self {
        Self { status, code }
    }

    fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized")
    }

    fn forbidden() -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden")
    }

    fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found")
    }

    fn internal() -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
    }
}

impl IntoResponse for HostedApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({
                "error": {
                    "code": self.code
                }
            })),
        )
            .into_response()
    }
}

pub fn hosted_relay_app(state: HostedRelayState) -> Router {
    let ingress = relay_ingress_app(RelayIngressState::new(state.broker.clone()));
    Router::new()
        .route("/_ortyo/health", get(health))
        .route("/healthz", get(health))
        .route("/_ortyo/admin/workspaces", post(create_workspace))
        .route(
            "/_ortyo/admin/workspaces/{workspace_id}/credentials",
            post(issue_credential),
        )
        .route(
            "/_ortyo/hosted/exposures",
            get(list_hosted_exposures).post(provision_exposure),
        )
        .route(
            "/_ortyo/hosted/exposures/{exposure_id}",
            get(get_hosted_exposure).delete(revoke_hosted_exposure),
        )
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

async fn create_workspace(
    State(state): State<HostedRelayState>,
    headers: HeaderMap,
    Json(request): Json<CreateWorkspaceRequest>,
) -> Result<(StatusCode, Json<Workspace>), HostedApiError> {
    authorize_control(&state, &headers)?;
    let workspace = state
        .identities
        .create_workspace_async(request.slug)
        .await
        .map_err(identity_admin_error)?;
    Ok((StatusCode::CREATED, Json(workspace)))
}

async fn issue_credential(
    State(state): State<HostedRelayState>,
    Path(workspace_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<IssueCredentialRequest>,
) -> Result<(StatusCode, Json<IssuedApiCredential>), HostedApiError> {
    authorize_control(&state, &headers)?;
    let credential = state
        .identities
        .issue_credential_async(workspace_id, request.name, request.scopes)
        .await
        .map_err(identity_admin_error)?;
    Ok((StatusCode::CREATED, Json(credential)))
}

async fn provision_exposure(
    State(state): State<HostedRelayState>,
    headers: HeaderMap,
    Json(request): Json<ProvisionExposureRequest>,
) -> Result<(StatusCode, Json<ProvisionedExposure>), HostedApiError> {
    let authorization = authorize_api(&state, &headers, ApiScope::ExposuresCreate).await?;

    if request.name.trim().is_empty() || request.target_port == 0 {
        return Err(HostedApiError::new(StatusCode::BAD_REQUEST, "invalid_exposure"));
    }

    let exposure_id = Uuid::now_v7();
    let capability = state
        .capabilities
        .issue_async(exposure_id, state.capability_ttl)
        .await
        .map_err(|_| HostedApiError::internal())?;
    let expires_at = capability
        .expires_at
        .duration_since(UNIX_EPOCH)
        .map_err(|_| HostedApiError::internal())?
        .as_secs();

    let public_url = format!("{}/e/{exposure_id}", state.public_base_url);
    let runtime_url = format!("{}/_ortyo/runtime/{exposure_id}", state.runtime_ws_base_url);

    if state
        .exposures
        .save_async(HostedExposureRecord {
            exposure_id,
            workspace_id: Some(authorization.workspace_id),
            name: request.name.clone(),
            target_port: request.target_port,
            public_url: public_url.clone(),
            runtime_url: runtime_url.clone(),
            capability_expires_at_unix_seconds: expires_at,
            revoked: false,
        })
        .await
        .is_err()
    {
        let _ = state.capabilities.revoke_async(&capability.token).await;
        return Err(HostedApiError::internal());
    }

    Ok((
        StatusCode::CREATED,
        Json(ProvisionedExposure {
            exposure_id,
            workspace_id: Some(authorization.workspace_id),
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

async fn list_hosted_exposures(
    State(state): State<HostedRelayState>,
    headers: HeaderMap,
) -> Result<Json<Vec<HostedExposureRecord>>, HostedApiError> {
    let authorization = authorize_api(&state, &headers, ApiScope::ExposuresRead).await?;
    let exposures = state
        .exposures
        .list_for_workspace_async(authorization.workspace_id)
        .await
        .map_err(|_| HostedApiError::internal())?;
    Ok(Json(exposures))
}

async fn get_hosted_exposure(
    State(state): State<HostedRelayState>,
    Path(exposure_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<HostedExposureRecord>, HostedApiError> {
    let authorization = authorize_api(&state, &headers, ApiScope::ExposuresRead).await?;
    let exposure = owned_exposure(&state, exposure_id, authorization.workspace_id).await?;
    Ok(Json(exposure))
}

async fn revoke_hosted_exposure(
    State(state): State<HostedRelayState>,
    Path(exposure_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<StatusCode, HostedApiError> {
    let authorization = authorize_api(&state, &headers, ApiScope::ExposuresRevoke).await?;
    owned_exposure(&state, exposure_id, authorization.workspace_id).await?;

    state
        .capabilities
        .revoke_exposure_async(exposure_id)
        .await
        .map_err(|_| HostedApiError::internal())?;
    state
        .exposures
        .revoke_async(exposure_id)
        .await
        .map_err(|_| HostedApiError::internal())?;
    state.broker.disconnect(exposure_id).await;
    Ok(StatusCode::NO_CONTENT)
}

fn authorize_control(
    state: &HostedRelayState,
    headers: &HeaderMap,
) -> Result<(), HostedApiError> {
    let token = bearer_token(headers).ok_or_else(HostedApiError::unauthorized)?;
    if token_digest(token) != state.control_token_digest {
        return Err(HostedApiError::unauthorized());
    }
    Ok(())
}

async fn authorize_api(
    state: &HostedRelayState,
    headers: &HeaderMap,
    required_scope: ApiScope,
) -> Result<ApiAuthorization, HostedApiError> {
    let token = bearer_token(headers)
        .ok_or_else(HostedApiError::unauthorized)?
        .to_owned();
    state
        .identities
        .authorize_async(token, required_scope)
        .await
        .map_err(identity_authorization_error)
}

fn identity_authorization_error(error: IdentityError) -> HostedApiError {
    match error {
        IdentityError::Forbidden => HostedApiError::forbidden(),
        IdentityError::InvalidCredential
        | IdentityError::RevokedCredential
        | IdentityError::WorkspaceDisabled
        | IdentityError::WorkspaceNotFound => HostedApiError::unauthorized(),
        IdentityError::InvalidWorkspace
        | IdentityError::DuplicateWorkspace
        | IdentityError::Storage(_) => HostedApiError::internal(),
    }
}

fn identity_admin_error(error: IdentityError) -> HostedApiError {
    match error {
        IdentityError::InvalidWorkspace | IdentityError::InvalidCredential => {
            HostedApiError::new(StatusCode::BAD_REQUEST, "invalid_request")
        }
        IdentityError::DuplicateWorkspace => {
            HostedApiError::new(StatusCode::CONFLICT, "workspace_exists")
        }
        IdentityError::WorkspaceNotFound => HostedApiError::not_found(),
        IdentityError::WorkspaceDisabled => {
            HostedApiError::new(StatusCode::CONFLICT, "workspace_disabled")
        }
        IdentityError::Forbidden | IdentityError::RevokedCredential => HostedApiError::forbidden(),
        IdentityError::Storage(_) => HostedApiError::internal(),
    }
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
    active_hosted_exposure(&state, exposure_id).await?;
    state
        .capabilities
        .authorize_async(exposure_id, capability)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    state
        .capabilities
        .revoke_async(capability)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    state
        .exposures
        .revoke_async(exposure_id)
        .await
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
    active_hosted_exposure(&state, exposure_id).await?;

    state
        .capabilities
        .authorize_async(exposure_id, capability)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let broker = state.broker.clone();
    Ok(ws
        .max_message_size(2 * 1024 * 1024)
        .on_upgrade(move |socket| async move {
            let _ = serve_websocket(socket, broker, exposure_id).await;
        }))
}

async fn owned_exposure(
    state: &HostedRelayState,
    exposure_id: Uuid,
    workspace_id: Uuid,
) -> Result<HostedExposureRecord, HostedApiError> {
    let exposure = state
        .exposures
        .get_async(exposure_id)
        .await
        .map_err(|_| HostedApiError::internal())?
        .ok_or_else(HostedApiError::not_found)?;
    if exposure.workspace_id != Some(workspace_id) {
        return Err(HostedApiError::not_found());
    }
    Ok(exposure)
}

async fn active_hosted_exposure(
    state: &HostedRelayState,
    exposure_id: Uuid,
) -> Result<HostedExposureRecord, StatusCode> {
    let exposure = state
        .exposures
        .get_async(exposure_id)
        .await
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
