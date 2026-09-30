use std::time::Instant;

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{OriginalUri, Path, State},
    http::{
        HeaderMap, Method, StatusCode,
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
    domain::{Direction, Exposure, Interaction, Origin, Protocol, Recording, Session},
    exposure::{ExposureError, ExposureService},
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
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/_ortyo/interactions", get(list_interactions))
        .route("/_ortyo/recordings", post(create_recording))
        .route("/_ortyo/recordings/{id}/replay", post(replay_recording))
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
    let recording = Recording {
        id: Uuid::now_v7(),
        created_at: Utc::now(),
        interaction_ids: state.store.all().into_iter().map(|item| item.id).collect(),
    };
    state.store.save_recording(&recording);
    Json(recording)
}

async fn replay_recording(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<Interaction>>, StatusCode> {
    let recording = state.store.recording(id).ok_or(StatusCode::NOT_FOUND)?;
    let mut replayed = Vec::new();

    for source_id in recording.interaction_ids {
        let source = state.store.find(source_id).ok_or(StatusCode::CONFLICT)?;
        let interaction = Interaction {
            id: Uuid::now_v7(),
            session_id: state.session.id,
            protocol: source.protocol,
            direction: source.direction,
            origin: Origin::Replayed,
            operation: source.operation,
            started_at: Utc::now(),
            duration_ms: 0,
            request: source.request,
            response: source.response,
            source_interaction_id: Some(source.id),
        };
        state.store.record(interaction.clone());
        replayed.push(interaction);
    }

    Ok(Json(replayed))
}

async fn create_exposure(
    State(state): State<AppState>,
    Json(request): Json<CreateExposureRequest>,
) -> Result<(StatusCode, Json<Exposure>), StatusCode> {
    state
        .exposures
        .create(state.session.id, request.name, request.port)
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
    let exposure = state
        .exposures
        .active(id)
        .map_err(exposure_error_status)?;

    let started = Instant::now();
    let started_at = Utc::now();
    let target_path = format!("/{path}");
    let target_url = match uri.query() {
        Some(query) => format!(
            "http://{}:{}{}?{}",
            exposure.target.host, exposure.target.port, target_path, query
        ),
        None => format!(
            "http://{}:{}{}",
            exposure.target.host, exposure.target.port, target_path
        ),
    };

    let mut outgoing = state.client.request(method.clone(), &target_url).body(body.clone());
    for (name, value) in &headers {
        if name != HOST && name != CONTENT_LENGTH && name != CONNECTION && name != TRANSFER_ENCODING {
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
        operation: format!("{} {}", method, target_path),
        started_at,
        duration_ms: started.elapsed().as_millis() as u64,
        request: json!({
            "exposure_id": id,
            "method": method.as_str(),
            "path": target_path,
            "query": uri.query(),
            "headers": headers_to_json(&headers),
            "body": String::from_utf8_lossy(&body)
        }),
        response: json!({
            "status": status.as_u16(),
            "headers": headers_to_json(&response_headers),
            "body": String::from_utf8_lossy(&response_body)
        }),
        source_interaction_id: None,
    });

    let mut response = Response::builder().status(status);
    for (name, value) in &response_headers {
        if name != CONTENT_LENGTH && name != CONNECTION && name != TRANSFER_ENCODING {
            response = response.header(name, value);
        }
    }

    response
        .body(Body::from(response_body))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn exposure_error_status(error: ExposureError) -> StatusCode {
    match error {
        ExposureError::InvalidName
        | ExposureError::InvalidPort
        | ExposureError::DuplicateName => StatusCode::BAD_REQUEST,
        ExposureError::NotFound => StatusCode::NOT_FOUND,
        ExposureError::Inactive => StatusCode::GONE,
        ExposureError::Provider(_) => StatusCode::BAD_GATEWAY,
    }
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
