use std::time::Duration;

use axum::{
    Router,
    body::{Body, Bytes},
    extract::{OriginalUri, Path, State},
    http::{
        HeaderMap, HeaderName, HeaderValue, Method, StatusCode,
        header::{CONNECTION, CONTENT_LENGTH, HOST, TRANSFER_ENCODING},
    },
    response::Response,
    routing::any,
};
use uuid::Uuid;

use crate::relay::{RelayBroker, RelayError, RelayRequest, RelayResponse};

#[derive(Clone)]
pub struct RelayIngressState {
    pub broker: RelayBroker,
    pub timeout: Duration,
    pub max_body_bytes: usize,
}

impl RelayIngressState {
    pub fn new(broker: RelayBroker) -> Self {
        Self {
            broker,
            timeout: Duration::from_secs(30),
            max_body_bytes: 1024 * 1024,
        }
    }
}

pub fn relay_ingress_app(state: RelayIngressState) -> Router {
    Router::new()
        .route("/e/{exposure_id}", any(ingress_root))
        .route("/e/{exposure_id}/{*path}", any(ingress_path))
        .with_state(state)
}

async fn ingress_root(
    State(state): State<RelayIngressState>,
    Path(exposure_id): Path<Uuid>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    ingress(
        state,
        exposure_id,
        "/".to_owned(),
        uri.query(),
        method,
        headers,
        body,
    )
    .await
}

async fn ingress_path(
    State(state): State<RelayIngressState>,
    Path((exposure_id, path)): Path<(Uuid, String)>,
    OriginalUri(uri): OriginalUri,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    ingress(
        state,
        exposure_id,
        format!("/{path}"),
        uri.query(),
        method,
        headers,
        body,
    )
    .await
}

async fn ingress(
    state: RelayIngressState,
    exposure_id: Uuid,
    path: String,
    query: Option<&str>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, StatusCode> {
    if body.len() > state.max_body_bytes {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    let response = state
        .broker
        .ingress_with_timeout(
            RelayRequest {
                id: Uuid::now_v7(),
                exposure_id,
                method: method.as_str().to_owned(),
                path,
                query: query.map(ToOwned::to_owned),
                headers: request_headers(&headers),
                body: body.to_vec(),
            },
            state.timeout,
        )
        .await
        .map_err(relay_error_status)?;

    relay_response(response)
}

fn request_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .filter(|(name, _)| {
            *name != HOST
                && *name != CONTENT_LENGTH
                && *name != CONNECTION
                && *name != TRANSFER_ENCODING
        })
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.to_string(), value.to_owned()))
        })
        .collect()
}

fn relay_response(response: RelayResponse) -> Result<Response, StatusCode> {
    let status = StatusCode::from_u16(response.status).map_err(|_| StatusCode::BAD_GATEWAY)?;
    let mut builder = Response::builder().status(status);

    for (name, value) in response.headers {
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| StatusCode::BAD_GATEWAY)?;
        if name == CONTENT_LENGTH || name == CONNECTION || name == TRANSFER_ENCODING {
            continue;
        }
        let value = HeaderValue::from_str(&value).map_err(|_| StatusCode::BAD_GATEWAY)?;
        builder = builder.header(name, value);
    }

    builder
        .body(Body::from(response.body))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

fn relay_error_status(error: RelayError) -> StatusCode {
    match error {
        RelayError::RuntimeUnavailable | RelayError::RuntimeDisconnected => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        RelayError::Timeout => StatusCode::GATEWAY_TIMEOUT,
        RelayError::ResponseDropped => StatusCode::BAD_GATEWAY,
        RelayError::Overloaded => StatusCode::SERVICE_UNAVAILABLE,
    }
}
