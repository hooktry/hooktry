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
    agent_surface,
    anonymous::{AnonymousExposureService, AnonymousExposureStore, anonymous_app},
    approval::{ApprovalDecision, ApprovalError, ApprovalRecord, ApprovalStore},
    domain::{ExposureAccess, ExposureMode},
    execution::{
        ExecutionError, ExecutionEvidence, ExecutionProviderKind, ExecutionRecord,
        HttpExecutionProvider, HttpExecutionRequest,
    },
    execution_store::{DurableExecutionRecord, ExecutionStore, ExecutionStoreError},
    hosted_identity::{
        ApiAuthorization, ApiScope, HostedIdentityStore, IdentityError, IssuedApiCredential,
        Workspace,
    },
    hosted_state::{HostedExposureRecord, HostedExposureStore},
    relay::RelayBroker,
    relay_auth::{CapabilityStore, token_digest},
    relay_ingress::{RelayIngressState, relay_ingress_app},
    secret::SecretStore,
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
    pub executor: HttpExecutionProvider,
    pub approvals: ApprovalStore,
    pub executions: ExecutionStore,
    pub anonymous: AnonymousExposureService,
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
        let anonymous = AnonymousExposureService::new(
            AnonymousExposureStore::default(),
            public_base_url.clone(),
        );
        Self {
            broker,
            capabilities,
            identities: HostedIdentityStore::default(),
            public_base_url,
            relay_addr: Some(relay_addr.into()),
            runtime_ws_base_url,
            capability_ttl: Duration::from_secs(15 * 60),
            exposures: HostedExposureStore::default(),
            executor: HttpExecutionProvider::new(SecretStore::default()),
            approvals: ApprovalStore::default(),
            executions: ExecutionStore::default(),
            anonymous,
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
            SecretStore::default(),
            public_base_url,
            control_token,
        )
    }

    pub fn websocket_only_with_stores(
        broker: RelayBroker,
        capabilities: CapabilityStore,
        exposures: HostedExposureStore,
        identities: HostedIdentityStore,
        secrets: SecretStore,
        public_base_url: impl Into<String>,
        control_token: &str,
    ) -> Self {
        let public_base_url = public_base_url.into().trim_end_matches('/').to_owned();
        let runtime_ws_base_url = websocket_base_url(&public_base_url);
        let anonymous = AnonymousExposureService::new(
            AnonymousExposureStore::default(),
            public_base_url.clone(),
        );
        Self {
            broker,
            capabilities,
            identities,
            public_base_url,
            relay_addr: None,
            runtime_ws_base_url,
            capability_ttl: Duration::from_secs(15 * 60),
            exposures,
            executor: HttpExecutionProvider::new(secrets),
            approvals: ApprovalStore::default(),
            executions: ExecutionStore::default(),
            control_token_digest: token_digest(control_token),
        }
    }

    pub fn with_anonymous_store(mut self, store: AnonymousExposureStore) -> Self {
        self.anonymous = self.anonymous.with_store(store);
        self
    }

    pub fn with_approval_store(mut self, approvals: ApprovalStore) -> Self {
        self.approvals = approvals;
        self
    }

    pub fn with_execution_store(mut self, executions: ExecutionStore) -> Self {
        self.executions = executions;
        self
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
struct BootstrapRequest {
    slug: String,
    #[serde(default = "default_bootstrap_credential_name")]
    credential_name: String,
}

#[derive(Debug, Serialize)]
struct BootstrapResponse {
    workspace: Workspace,
    credential: IssuedApiCredential,
}

#[derive(Debug, Deserialize)]
struct ApprovalDecisionRequest {
    decision: ApprovalDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApprovedExecution {
    pub approval: ApprovalRecord,
    pub execution: ExecutionRecord,
}

fn default_bootstrap_credential_name() -> String {
    "initial-cli".to_owned()
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
    execution_id: Option<Uuid>,
}

impl HostedApiError {
    fn new(status: StatusCode, code: &'static str) -> Self {
        Self {
            status,
            code,
            execution_id: None,
        }
    }

    fn with_execution_id(mut self, execution_id: Uuid) -> Self {
        self.execution_id = Some(execution_id);
        self
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
        let mut error = serde_json::json!({
            "code": self.code
        });
        if let Some(execution_id) = self.execution_id {
            error["execution_id"] = serde_json::json!(execution_id);
        }
        (
            self.status,
            Json(serde_json::json!({
                "error": error
            })),
        )
            .into_response()
    }
}

pub fn hosted_relay_app(state: HostedRelayState) -> Router {
    let ingress = relay_ingress_app(RelayIngressState::new(state.broker.clone()));
    let anonymous = anonymous_app(state.anonymous.clone(), state.identities.clone());
    Router::new()
        .route("/llms.txt", get(agent_surface::llms_txt))
        .route("/llms-full.txt", get(agent_surface::llms_full_txt))
        .route("/skills/ortyo/SKILL.md", get(agent_surface::skill_md))
        .route("/_ortyo/health", get(health))
        .route("/healthz", get(health))
        .route("/_ortyo/bootstrap", post(bootstrap_first_workspace))
        .route("/_ortyo/admin/workspaces", post(create_workspace))
        .route(
            "/_ortyo/admin/workspaces/{workspace_id}/credentials",
            post(issue_credential),
        )
        .route("/_ortyo/hosted/execute", post(execute_http))
        .route(
            "/_ortyo/hosted/executions/{execution_id}",
            get(get_execution),
        )
        .route(
            "/_ortyo/hosted/approvals",
            get(list_pending_approvals).post(create_approval),
        )
        .route("/_ortyo/hosted/approvals/{approval_id}", get(get_approval))
        .route(
            "/_ortyo/hosted/approvals/{approval_id}/decision",
            post(decide_approval),
        )
        .route(
            "/_ortyo/hosted/approvals/{approval_id}/execute",
            post(execute_approved),
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
        .merge(anonymous)
}

async fn health() -> Json<serde_json::Value> {
    let revision = std::env::var("RENDER_GIT_COMMIT")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    Json(serde_json::json!({
        "ok": true,
        "service": "hosted_relay",
        "revision": revision
    }))
}

async fn bootstrap_first_workspace(
    State(state): State<HostedRelayState>,
    Json(request): Json<BootstrapRequest>,
) -> Result<(StatusCode, Json<BootstrapResponse>), HostedApiError> {
    let (workspace, credential) = state
        .identities
        .bootstrap_first_workspace_async(request.slug, request.credential_name)
        .await
        .map_err(identity_bootstrap_error)?;
    Ok((
        StatusCode::CREATED,
        Json(BootstrapResponse {
            workspace,
            credential,
        }),
    ))
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

async fn list_pending_approvals(
    State(state): State<HostedRelayState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ApprovalRecord>>, HostedApiError> {
    let authorization = authorize_api(&state, &headers, ApiScope::RequestsApprove).await?;
    let approvals = state
        .approvals
        .list_pending_async(authorization.workspace_id)
        .await
        .map_err(approval_error)?;
    Ok(Json(approvals))
}

async fn create_approval(
    State(state): State<HostedRelayState>,
    headers: HeaderMap,
    Json(request): Json<HttpExecutionRequest>,
) -> Result<(StatusCode, Json<ApprovalRecord>), HostedApiError> {
    let authorization = authorize_api(&state, &headers, ApiScope::RequestsExecute).await?;
    let approval = state
        .approvals
        .create_async(
            authorization.workspace_id,
            authorization.credential_id,
            request,
        )
        .await
        .map_err(approval_error)?;
    Ok((StatusCode::CREATED, Json(approval)))
}

async fn get_approval(
    State(state): State<HostedRelayState>,
    Path(approval_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<ApprovalRecord>, HostedApiError> {
    let authorization = authorize_api_any(
        &state,
        &headers,
        &[ApiScope::RequestsExecute, ApiScope::RequestsApprove],
    )
    .await?;
    let approval = state
        .approvals
        .get_async(authorization.workspace_id, approval_id)
        .await
        .map_err(approval_error)?
        .ok_or_else(HostedApiError::not_found)?;
    Ok(Json(approval))
}

async fn decide_approval(
    State(state): State<HostedRelayState>,
    Path(approval_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<ApprovalDecisionRequest>,
) -> Result<Json<ApprovalRecord>, HostedApiError> {
    let authorization = authorize_api(&state, &headers, ApiScope::RequestsApprove).await?;
    let approval = state
        .approvals
        .decide_async(
            authorization.workspace_id,
            approval_id,
            authorization.credential_id,
            request.decision,
        )
        .await
        .map_err(approval_error)?;
    Ok(Json(approval))
}

async fn execute_approved(
    State(state): State<HostedRelayState>,
    Path(approval_id): Path<Uuid>,
    headers: HeaderMap,
    Json(request): Json<HttpExecutionRequest>,
) -> Result<Json<ApprovedExecution>, HostedApiError> {
    let authorization = authorize_api(&state, &headers, ApiScope::RequestsExecute).await?;
    let execution_id = Uuid::now_v7();
    let reserved = state
        .executions
        .reserve_async(
            authorization.workspace_id,
            execution_id,
            ExecutionProviderKind::Http,
        )
        .await
        .map_err(execution_store_error)?;

    let approval = match state
        .approvals
        .consume_async(
            authorization.workspace_id,
            approval_id,
            authorization.credential_id,
            execution_id,
            request.clone(),
        )
        .await
    {
        Ok(approval) => approval,
        Err(error) => {
            let _ = state
                .executions
                .discard_started_async(authorization.workspace_id, execution_id)
                .await;
            return Err(approval_error(error));
        }
    };

    let execution = state
        .executor
        .execute_recorded_with_envelope(
            authorization.workspace_id,
            request,
            execution_id,
            reserved.started_at_unix_ms,
        )
        .await;
    state
        .executions
        .complete_async(execution.clone())
        .await
        .map_err(execution_store_error)?;

    Ok(Json(ApprovedExecution {
        approval,
        execution,
    }))
}

async fn execute_http(
    State(state): State<HostedRelayState>,
    headers: HeaderMap,
    Json(request): Json<HttpExecutionRequest>,
) -> Result<(StatusCode, Json<ExecutionEvidence>), HostedApiError> {
    let authorization = authorize_api(&state, &headers, ApiScope::RequestsExecute).await?;
    let execution_id = Uuid::now_v7();
    let reserved = state
        .executions
        .reserve_async(
            authorization.workspace_id,
            execution_id,
            ExecutionProviderKind::Http,
        )
        .await
        .map_err(execution_store_error)?;
    let execution = state
        .executor
        .execute_recorded_with_envelope(
            authorization.workspace_id,
            request,
            execution_id,
            reserved.started_at_unix_ms,
        )
        .await;
    state
        .executions
        .complete_async(execution.clone())
        .await
        .map_err(execution_store_error)?;
    let evidence = execution
        .into_result()
        .map_err(|error| execution_error(error).with_execution_id(execution_id))?;
    Ok((StatusCode::OK, Json(evidence)))
}

async fn get_execution(
    State(state): State<HostedRelayState>,
    Path(execution_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Json<DurableExecutionRecord>, HostedApiError> {
    let authorization = authorize_api(&state, &headers, ApiScope::RequestsExecute).await?;
    let execution = state
        .executions
        .get_async(authorization.workspace_id, execution_id)
        .await
        .map_err(execution_store_error)?
        .ok_or_else(HostedApiError::not_found)?;
    Ok(Json(execution))
}

fn execution_store_error(error: ExecutionStoreError) -> HostedApiError {
    match error {
        ExecutionStoreError::NotFound => HostedApiError::not_found(),
        ExecutionStoreError::AlreadyCompleted
        | ExecutionStoreError::InvalidRecord(_)
        | ExecutionStoreError::Storage(_) => HostedApiError::internal(),
    }
}

fn execution_error(error: ExecutionError) -> HostedApiError {
    match error {
        ExecutionError::InvalidRequest => {
            HostedApiError::new(StatusCode::BAD_REQUEST, "invalid_request")
        }
        ExecutionError::UnsafeDestination => {
            HostedApiError::new(StatusCode::FORBIDDEN, "unsafe_destination")
        }
        ExecutionError::SecretDestinationDenied => {
            HostedApiError::new(StatusCode::FORBIDDEN, "secret_destination_denied")
        }
        ExecutionError::SecretNotFound => {
            HostedApiError::new(StatusCode::BAD_REQUEST, "secret_not_found")
        }
        ExecutionError::ResponseTooLarge => {
            HostedApiError::new(StatusCode::BAD_GATEWAY, "response_too_large")
        }
        ExecutionError::CaptureFailed => {
            HostedApiError::new(StatusCode::BAD_GATEWAY, "capture_failed")
        }
        ExecutionError::RequestFailed => {
            HostedApiError::new(StatusCode::BAD_GATEWAY, "request_failed")
        }
    }
}

fn approval_error(error: ApprovalError) -> HostedApiError {
    match error {
        ApprovalError::InvalidRequest => {
            HostedApiError::new(StatusCode::BAD_REQUEST, "invalid_approval_request")
        }
        ApprovalError::NotFound => HostedApiError::not_found(),
        ApprovalError::NotPending => {
            HostedApiError::new(StatusCode::CONFLICT, "approval_already_decided")
        }
        ApprovalError::Pending => HostedApiError::new(StatusCode::CONFLICT, "approval_pending"),
        ApprovalError::Denied => HostedApiError::new(StatusCode::FORBIDDEN, "approval_denied"),
        ApprovalError::Consumed => HostedApiError::new(StatusCode::CONFLICT, "approval_consumed"),
        ApprovalError::RequestMismatch => {
            HostedApiError::new(StatusCode::CONFLICT, "approval_request_mismatch")
        }
        ApprovalError::Storage(_) => HostedApiError::internal(),
    }
}

async fn provision_exposure(
    State(state): State<HostedRelayState>,
    headers: HeaderMap,
    Json(request): Json<ProvisionExposureRequest>,
) -> Result<(StatusCode, Json<ProvisionedExposure>), HostedApiError> {
    let authorization = authorize_api(&state, &headers, ApiScope::ExposuresCreate).await?;

    if request.name.trim().is_empty() || request.target_port == 0 {
        return Err(HostedApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_exposure",
        ));
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

fn authorize_control(state: &HostedRelayState, headers: &HeaderMap) -> Result<(), HostedApiError> {
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

async fn authorize_api_any(
    state: &HostedRelayState,
    headers: &HeaderMap,
    required_scopes: &[ApiScope],
) -> Result<ApiAuthorization, HostedApiError> {
    let token = bearer_token(headers)
        .ok_or_else(HostedApiError::unauthorized)?
        .to_owned();
    for scope in required_scopes {
        match state
            .identities
            .authorize_async(token.clone(), *scope)
            .await
        {
            Ok(authorization) => return Ok(authorization),
            Err(IdentityError::Forbidden) => continue,
            Err(error) => return Err(identity_authorization_error(error)),
        }
    }
    Err(HostedApiError::forbidden())
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
        | IdentityError::BootstrapAlreadyCompleted
        | IdentityError::Storage(_) => HostedApiError::internal(),
    }
}

fn identity_bootstrap_error(error: IdentityError) -> HostedApiError {
    match error {
        IdentityError::BootstrapAlreadyCompleted => {
            HostedApiError::new(StatusCode::CONFLICT, "bootstrap_already_completed")
        }
        IdentityError::InvalidWorkspace | IdentityError::InvalidCredential => {
            HostedApiError::new(StatusCode::BAD_REQUEST, "invalid_request")
        }
        IdentityError::DuplicateWorkspace
        | IdentityError::WorkspaceNotFound
        | IdentityError::WorkspaceDisabled
        | IdentityError::RevokedCredential
        | IdentityError::Forbidden
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
        IdentityError::BootstrapAlreadyCompleted => {
            HostedApiError::new(StatusCode::CONFLICT, "bootstrap_already_completed")
        }
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
