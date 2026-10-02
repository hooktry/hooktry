use axum::{
    Json,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

use crate::hosted::HostedRelayState;

const MODERN_PROTOCOL_VERSION: &str = "2026-07-28";
const LATEST_HANDSHAKE_PROTOCOL_VERSION: &str = "2025-11-25";
const LEGACY_HANDSHAKE_PROTOCOL_VERSION: &str = "2025-06-18";

pub async fn post(
    State(state): State<HostedRelayState>,
    Json(request): Json<Value>,
) -> Response {
    let response = match handle(&state, request).await {
        Ok(Some(body)) => (StatusCode::OK, Json(body)).into_response(),
        Ok(None) => StatusCode::ACCEPTED.into_response(),
        Err(message) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "jsonrpc": "2.0",
                "error": {
                    "code": -32603,
                    "message": message
                },
                "id": Value::Null
            })),
        )
            .into_response(),
    };
    with_cors(response)
}

pub async fn options() -> Response {
    with_cors(StatusCode::NO_CONTENT.into_response())
}

pub async fn method_not_allowed() -> Response {
    let mut response = StatusCode::METHOD_NOT_ALLOWED.into_response();
    response
        .headers_mut()
        .insert(header::ALLOW, HeaderValue::from_static("POST, OPTIONS"));
    with_cors(response)
}

pub async fn handle(
    state: &HostedRelayState,
    request: Value,
) -> Result<Option<Value>, String> {
    let id = request.get("id").cloned();
    let method = request
        .get("method")
        .and_then(Value::as_str)
        .ok_or_else(|| "JSON-RPC method is required".to_owned())?;

    if id.is_none() {
        return Ok(None);
    }
    let id = id.unwrap();

    let result = match method {
        "server/discover" => json!({
            "resultType": "complete",
            "supportedVersions": [MODERN_PROTOCOL_VERSION],
            "capabilities": {"tools": {}},
            "instructions": "Hooktry exposes a deliberately small public remote tool surface. Private workspace tools require OAuth and are not part of this slice.",
            "serverInfo": {
                "name": "hooktry",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
        "initialize" => json!({
            "protocolVersion": negotiated_handshake_protocol(&request),
            "capabilities": {"tools": {}},
            "serverInfo": {
                "name": "hooktry",
                "version": env!("CARGO_PKG_VERSION")
            },
            "instructions": "Hooktry exposes a deliberately small public remote tool surface. Private workspace tools require OAuth and are not part of this slice."
        }),
        "ping" => json!({}),
        "tools/list" => json!({"tools": tools()}),
        "tools/call" => {
            call_tool(
                state,
                request.get("params").cloned().unwrap_or(Value::Null),
            )
            .await?
        }
        _ => return Ok(Some(error(id, -32601, "method not found"))),
    };

    Ok(Some(json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })))
}

fn negotiated_handshake_protocol(request: &Value) -> &'static str {
    match request
        .pointer("/params/protocolVersion")
        .and_then(Value::as_str)
    {
        Some(LEGACY_HANDSHAKE_PROTOCOL_VERSION) => LEGACY_HANDSHAKE_PROTOCOL_VERSION,
        Some(LATEST_HANDSHAKE_PROTOCOL_VERSION) => LATEST_HANDSHAKE_PROTOCOL_VERSION,
        _ => LATEST_HANDSHAKE_PROTOCOL_VERSION,
    }
}

fn tools() -> Vec<Value> {
    vec![json!({
        "name": "create_webhook_endpoint",
        "title": "Create webhook endpoint",
        "description": "Create a temporary Hooktry webhook URL when the user needs an endpoint to receive test webhook or integration traffic. The endpoint expires automatically. This tool does not expose captured request data, viewer capabilities, or claim capabilities.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "required": [],
            "additionalProperties": false
        },
        "outputSchema": {
            "type": "object",
            "properties": {
                "exposure_id": {
                    "type": "string",
                    "format": "uuid"
                },
                "hook_url": {
                    "type": "string",
                    "format": "uri"
                },
                "expires_at_unix_seconds": {
                    "type": ["integer", "null"]
                },
                "request_limit": {
                    "type": "integer"
                },
                "max_body_bytes": {
                    "type": "integer"
                },
                "max_retained_bytes": {
                    "type": "integer"
                }
            },
            "required": [
                "exposure_id",
                "hook_url",
                "expires_at_unix_seconds",
                "request_limit",
                "max_body_bytes",
                "max_retained_bytes"
            ],
            "additionalProperties": false
        },
        "annotations": {
            "readOnlyHint": false,
            "destructiveHint": false,
            "openWorldHint": false,
            "idempotentHint": false
        },
        "securitySchemes": [
            {"type": "noauth"}
        ]
    })]
}

async fn call_tool(state: &HostedRelayState, params: Value) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "tools/call requires a tool name".to_owned())?;

    match name {
        "create_webhook_endpoint" => {
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            if arguments
                .as_object()
                .is_none_or(|arguments| !arguments.is_empty())
            {
                return Ok(tool_error(
                    "create_webhook_endpoint does not accept arguments".to_owned(),
                ));
            }

            let provision = state
                .anonymous
                .provision_ingress()
                .await
                .map_err(|error| format!("provision anonymous webhook endpoint: {error:?}"))?;
            let structured = serde_json::to_value(provision)
                .map_err(|error| format!("serialize webhook endpoint: {error}"))?;
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!(
                        "Created a temporary Hooktry webhook endpoint: {}",
                        structured["hook_url"].as_str().unwrap_or_default()
                    )
                }],
                "structuredContent": structured,
                "isError": false
            }))
        }
        _ => Ok(tool_error(format!("unknown tool: {name}"))),
    }
}

fn tool_error(message: String) -> Value {
    json!({
        "content": [{"type": "text", "text": message}],
        "isError": true
    })
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn with_cors(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("POST, GET, DELETE, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static(
            "authorization, content-type, mcp-protocol-version, mcp-session-id",
        ),
    );
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("Mcp-Session-Id"),
    );
    response
}
