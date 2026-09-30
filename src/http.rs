use std::time::Instant;

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{OriginalUri, Path, State},
    http::{
        HeaderMap, HeaderName, HeaderValue, Method, StatusCode,
        header::{CONNECTION, CONTENT_LENGTH, HOST, TRANSFER_ENCODING},
    },
    response::{IntoResponse, Response},
    routing::{any, get, post},
};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    contract::assert_interaction,
    domain::{
        AssertionResult, Contract, Direction, Exposure, ExposureAccess, ExposureMode,
        ExposureTarget, Interaction, Origin, Protocol, Recording, Scenario, ScenarioOutcome,
        ScenarioRun, Session,
    },
    exposure::{CreateExposure, ExposureError, ExposureService},
    recording::{ReplayError, replay, snapshot_all},
    relay::{RelayRequest, RelayResponse},
    scenario::{CreateScenario, ScenarioError, complete as complete_scenario_run, create as create_scenario_definition, start as start_scenario_run},
    store::InteractionStore,
};

#[derive(Clone)]
pub struct AppState {
    pub store: InteractionStore,
    pub session: Session,
    pub exposures: ExposureService,
    pub client: reqwest::Client,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            store: InteractionStore::default(),
            session: Session {
                id: Uuid::now_v7(),
                started_at: Utc::now(),
            },
            exposures: ExposureService::default(),
            client: reqwest::Client::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct CreateExposureRequest {
    name: String,
    port: u16,
    mode: Option<ExposureMode>,
    access: Option<ExposureAccess>,
}

#[derive(Debug, Deserialize)]
struct CreateContractRequest {
    name: String,
    operation: Option<Value>,
    request: Option<Value>,
    response: Option<Value>,
}

struct ForwardHttpRequest {
    method: Method,
    path: String,
    query: Option<String>,
    headers: HeaderMap,
    body: Bytes,
    relay_request_id: Option<Uuid>,
}

struct ProxiedHttpResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/_ortyo/interactions", get(list_interactions))
        .route("/_ortyo/recordings", post(create_recording))
        .route("/_ortyo/recordings/{id}/replay", post(replay_recording))
        .route("/_ortyo/contracts", post(create_contract))
        .route("/_ortyo/contracts/{id}", get(get_contract))
        .route(
            "/_ortyo/contracts/{contract_id}/assert/{interaction_id}",
            post(assert_contract),
        )
        .route("/_ortyo/assertions/{id}", get(get_assertion))
        .route("/_ortyo/scenarios", post(create_scenario))
        .route("/_ortyo/scenarios/{id}", get(get_scenario))
        .route("/_ortyo/scenarios/{id}/start", post(start_scenario))
        .route(
            "/_ortyo/scenario-runs/{id}/complete",
            post(complete_scenario),
        )
        .route(
            "/_ortyo/scenario-runs/{id}/outcome",
            get(get_scenario_outcome),
        )
        .route(
            "/_ortyo/exposures",
            get(list_exposures).post(create_exposure),
        )
        .route(
            "/_ortyo/exposures/{id}",
            get(get_exposure).delete(revoke_exposure),
        )
        .route("/exposed/{id}/{*path}", any(proxy_exposure))
        .route("/boundary/{*path}", any(capture))
        .with_state(state)
}

async fn list_interactions(State(state): State<AppState>) -> Json<Vec<Interaction>> {
    Json(state.store.all())
}

async fn capture(
    State(state): State<AppState>,
    Path(path): Path<String>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let started = Instant::now();
    let started_at = Utc::now();
    let request = json!({
        "method": method.as_str(),
        "path": format!("/{path}"),
        "headers": headers_to_json(&headers),
        "body": String::from_utf8_lossy(&body)
    });
    let response = json!({"status": StatusCode::OK.as_u16()});

    state.store.record(Interaction {
        id: Uuid::now_v7(),
        session_id: state.session.id,
        protocol: Protocol::Http,
        direction: Direction::Inbound,
        origin: Origin::Observed,
        operation: format!("{} /{}", method, path),
        started_at,
        duration_ms: started.elapsed().as_millis() as u64,
        request,
        response,
        source_interaction_id: None,
    });

    (StatusCode::OK, Json(json!({"ok": true})))
}

async fn create_recording(State(state): State<AppState>) -> Json<Recording> {
    Json(snapshot_all(&state.store))
}

async fn replay_recording(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Interaction>>, StatusCode> {
    replay(&state.store, state.session.id, id)
        .map(Json)
        .map_err(replay_error_status)
}

async fn create_contract(
    State(state): State<AppState>,
    Json(request): Json<CreateContractRequest>,
) -> (StatusCode, Json<Contract>) {
    let contract = Contract {
        id: Uuid::now_v7(),
        name: request.name,
        operation: request.operation,
        request: request.request,
        response: request.response,
    };
    state.store.save_contract(&contract);
    (StatusCode::CREATED, Json(contract))
}

async fn get_contract(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Contract>, StatusCode> {
    state
        .store
        .contract(id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn assert_contract(
    State(state): State<AppState>,
    Path((contract_id, interaction_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<AssertionResult>, StatusCode> {
    let contract = state
        .store
        .contract(contract_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    let interaction = state
        .store
        .find(interaction_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    let assertion = assert_interaction(&contract, &interaction);
    state.store.save_assertion(&assertion);
    Ok(Json(assertion))
}

async fn get_assertion(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<AssertionResult>, StatusCode> {
    state
        .store
        .assertion(id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn create_scenario(
    State(state): State<AppState>,
    Json(request): Json<CreateScenario>,
) -> Result<(StatusCode, Json<Scenario>), StatusCode> {
    create_scenario_definition(&state.store, request)
        .map(|scenario| (StatusCode::CREATED, Json(scenario)))
        .map_err(scenario_error_status)
}

async fn get_scenario(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Scenario>, StatusCode> {
    state
        .store
        .scenario(id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn start_scenario(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<(StatusCode, Json<ScenarioRun>), StatusCode> {
    start_scenario_run(&state.store, &state.exposures, state.session.id, id)
        .map(|run| (StatusCode::CREATED, Json(run)))
        .map_err(scenario_error_status)
}

async fn complete_scenario(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ScenarioOutcome>, StatusCode> {
    complete_scenario_run(&state.store, &state.exposures, state.session.id, id)
        .map(Json)
        .map_err(scenario_error_status)
}

async fn get_scenario_outcome(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ScenarioOutcome>, StatusCode> {
    state
        .store
        .scenario_outcome(id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn create_exposure(
    State(state): State<AppState>,
    Json(request): Json<CreateExposureRequest>,
) -> Result<(StatusCode, Json<Exposure>), StatusCode> {
    let create = CreateExposure {
        name: request.name,
        target: ExposureTarget {
            host: "127.0.0.1".to_owned(),
            port: request.port,
        },
        mode: request.mode.unwrap_or(ExposureMode::Forward),
        access: request.access.unwrap_or(ExposureAccess::Private),
    };

    state
        .exposures
        .create_with(state.session.id, create)
        .map(|exposure| (StatusCode::CREATED, Json(exposure)))
        .map_err(exposure_error_status)
}

async fn list_exposures(State(state): State<AppState>) -> Json<Vec<Exposure>> {
    Json(state.exposures.all())
}

async fn get_exposure(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Exposure>, StatusCode> {
    state
        .exposures
        .get(id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn revoke_exposure(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Exposure>, StatusCode> {
    state
        .exposures
        .revoke(id)
        .map(Json)
        .map_err(exposure_error_status)
}

async fn proxy_exposure(
    State(state): State<AppState>,
    Path((id, path)): Path<(Uuid, String)>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    let exposure = state.exposures.active(id).map_err(exposure_error_status)?;
    let proxied = forward_and_record(
        &state,
        &exposure,
        ForwardHttpRequest {
            method,
            path: format!("/{path}"),
            query: uri.query().map(ToOwned::to_owned),
            headers,
            body,
            relay_request_id: None,
        },
    )
    .await?;

    build_response(proxied)
}

pub async fn proxy_relay_request(
    state: &AppState,
    request: &RelayRequest,
) -> Result<RelayResponse, StatusCode> {
    let exposure = state
        .exposures
        .active(request.exposure_id)
        .map_err(exposure_error_status)?;
    if exposure.mode != ExposureMode::Relay {
        return Err(StatusCode::CONFLICT);
    }

    let method =
        Method::from_bytes(request.method.as_bytes()).map_err(|_| StatusCode::BAD_REQUEST)?;
    let mut headers = HeaderMap::new();
    for (name, value) in &request.headers {
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| StatusCode::BAD_REQUEST)?;
        let value = HeaderValue::from_str(value).map_err(|_| StatusCode::BAD_REQUEST)?;
        headers.append(name, value);
    }

    let path = if request.path.starts_with('/') {
        request.path.clone()
    } else {
        format!("/{}", request.path)
    };

    let proxied = forward_and_record(
        state,
        &exposure,
        ForwardHttpRequest {
            method,
            path,
            query: request.query.clone(),
            headers,
            body: Bytes::from(request.body.clone()),
            relay_request_id: Some(request.id),
        },
    )
    .await?;

    Ok(RelayResponse {
        request_id: request.id,
        status: proxied.status.as_u16(),
        headers: headers_to_pairs(&proxied.headers),
        body: proxied.body.to_vec(),
    })
}

async fn forward_and_record(
    state: &AppState,
    exposure: &Exposure,
    request: ForwardHttpRequest,
) -> Result<ProxiedHttpResponse, StatusCode> {
    let started = Instant::now();
    let started_at = Utc::now();
    let target_url = match request.query.as_deref() {
        Some(query) => format!(
            "http://{}:{}{}?{}",
            exposure.target.host, exposure.target.port, request.path, query
        ),
        None => format!(
            "http://{}:{}{}",
            exposure.target.host, exposure.target.port, request.path
        ),
    };

    let mut outgoing = state
        .client
        .request(request.method.clone(), &target_url)
        .body(request.body.clone());
    for (name, value) in &request.headers {
        if name != HOST && name != CONTENT_LENGTH && name != CONNECTION && name != TRANSFER_ENCODING
        {
            outgoing = outgoing.header(name, value);
        }
    }

    let target_response = outgoing.send().await.map_err(|_| StatusCode::BAD_GATEWAY)?;
    let status = target_response.status();
    let response_headers = target_response.headers().clone();
    let response_body = target_response
        .bytes()
        .await
        .map_err(|_| StatusCode::BAD_GATEWAY)?;

    state.store.record(Interaction {
        id: Uuid::now_v7(),
        session_id: state.session.id,
        protocol: Protocol::Http,
        direction: Direction::Inbound,
        origin: Origin::Proxied,
        operation: format!("{} {}", request.method, request.path),
        started_at,
        duration_ms: started.elapsed().as_millis() as u64,
        request: json!({
            "exposure_id": exposure.id,
            "relay_request_id": request.relay_request_id,
            "method": request.method.as_str(),
            "path": request.path,
            "query": request.query,
            "headers": headers_to_json(&request.headers),
            "body": String::from_utf8_lossy(&request.body)
        }),
        response: json!({
            "status": status.as_u16(),
            "headers": headers_to_json(&response_headers),
            "body": String::from_utf8_lossy(&response_body)
        }),
        source_interaction_id: None,
    });

    Ok(ProxiedHttpResponse {
        status,
        headers: response_headers,
        body: response_body,
    })
}

fn build_response(proxied: ProxiedHttpResponse) -> Result<Response, StatusCode> {
    let mut response = Response::builder().status(proxied.status);
    for (name, value) in &proxied.headers {
        if name != CONTENT_LENGTH && name != CONNECTION && name != TRANSFER_ENCODING {
            response = response.header(name, value);
        }
    }

    response
        .body(Body::from(proxied.body))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn scenario_error_status(error: ScenarioError) -> StatusCode {
    match error {
        ScenarioError::InvalidName
        | ScenarioError::InvalidPort
        | ScenarioError::ContractRequired
        | ScenarioError::InvalidContractName
        | ScenarioError::InvalidOperation => StatusCode::BAD_REQUEST,
        ScenarioError::ScenarioNotFound | ScenarioError::RunNotFound => StatusCode::NOT_FOUND,
        ScenarioError::ContractNotFound(_) => StatusCode::CONFLICT,
        ScenarioError::Exposure(error) => exposure_error_status(error),
        ScenarioError::Replay(error) => replay_error_status(error),
    }
}

fn replay_error_status(error: ReplayError) -> StatusCode {
    match error {
        ReplayError::RecordingNotFound => StatusCode::NOT_FOUND,
        ReplayError::SourceInteractionNotFound(_) => StatusCode::CONFLICT,
    }
}

fn exposure_error_status(error: ExposureError) -> StatusCode {
    match error {
        ExposureError::InvalidName | ExposureError::InvalidPort | ExposureError::DuplicateName => {
            StatusCode::BAD_REQUEST
        }
        ExposureError::NotFound => StatusCode::NOT_FOUND,
        ExposureError::Inactive => StatusCode::GONE,
        ExposureError::Provider(_) => StatusCode::BAD_GATEWAY,
    }
}

fn headers_to_pairs(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            (
                name.to_string(),
                value.to_str().unwrap_or("<binary>").to_owned(),
            )
        })
        .collect()
}

fn headers_to_json(headers: &HeaderMap) -> Value {
    Value::Object(
        headers
            .iter()
            .map(|(name, value)| {
                (
                    name.to_string(),
                    Value::String(value.to_str().unwrap_or("<binary>").to_owned()),
                )
            })
            .collect(),
    )
}
