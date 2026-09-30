use std::time::Instant;

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode},
    response::IntoResponse,
    routing::{any, get},
};
use chrono::Utc;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    domain::{Direction, Interaction, Origin, Protocol, Session},
    store::InteractionStore,
};

#[derive(Clone)]
pub struct AppState {
    pub store: InteractionStore,
    pub session: Session,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            store: InteractionStore::default(),
            session: Session {
                id: Uuid::now_v7(),
                started_at: Utc::now(),
            },
        }
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/_ortyo/interactions", get(list_interactions))
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
    });

    (StatusCode::OK, Json(json!({"ok": true})))
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
