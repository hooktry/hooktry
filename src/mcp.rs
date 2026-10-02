use std::io::{BufRead, Write};

use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    approval::ApprovalDecision, execution::HttpExecutionRequest, hosted_client::HostedClient,
};

pub async fn run_stdio(base_url: &str) -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = line.map_err(|error| error.to_string())?;
        if line.trim().is_empty() {
            continue;
        }

        let request: Value =
            serde_json::from_str(&line).map_err(|error| format!("invalid JSON-RPC: {error}"))?;
        if let Some(response) = handle(base_url, request).await? {
            writeln!(stdout, "{}", response).map_err(|error| error.to_string())?;
            stdout.flush().map_err(|error| error.to_string())?;
        }
    }

    Ok(())
}

pub async fn handle(base_url: &str, request: Value) -> Result<Option<Value>, String> {
    let hosted = HostedClient::from_env(base_url);
    handle_with_hosted_client(base_url, &hosted, request).await
}

#[doc(hidden)]
pub async fn handle_with_hosted_client(
    base_url: &str,
    hosted: &HostedClient,
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
        "initialize" => json!({
            "protocolVersion": "2025-06-18",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "hooktry", "version": env!("CARGO_PKG_VERSION")}
        }),
        "tools/list" => json!({"tools": tools()}),
        "tools/call" => {
            call_tool(
                base_url,
                hosted,
                request.get("params").cloned().unwrap_or(Value::Null),
            )
            .await?
        }
        _ => return Ok(Some(error(id, -32601, "method not found"))),
    };

    Ok(Some(json!({"jsonrpc": "2.0", "id": id, "result": result})))
}

fn tools() -> Vec<Value> {
    vec![
        tool(
            "exposure_create",
            "Create a local HTTP Exposure for an upstream port.",
            json!({
                "name": {"type": "string"},
                "port": {"type": "integer", "minimum": 1, "maximum": 65535}
            }),
        ),
        tool(
            "exposure_get",
            "Get an Exposure by ID.",
            uuid_schema("exposure_id"),
        ),
        tool(
            "exposure_revoke",
            "Revoke an active Exposure by ID.",
            uuid_schema("exposure_id"),
        ),
        tool(
            "approval_inbox",
            "List pending approvals for the approver's Workspace, oldest first. Requires HOOKTRY_APPROVER_TOKEN.",
            json!({}),
        ),
        tool_with_required(
            "approval_create",
            "Ask for one exact hosted HTTP action. Returns a redacted pending ApprovalRecord.",
            json!({
                "request": http_execution_request_schema()
            }),
            &["request"],
        ),
        tool(
            "approval_get",
            "Inspect one workspace-scoped ApprovalRecord by ID.",
            uuid_schema("approval_id"),
        ),
        tool_with_required(
            "approval_decide",
            "Approve or deny a pending ApprovalRecord. Requires HOOKTRY_APPROVER_TOKEN in the MCP process environment; never falls back to HOOKTRY_TOKEN.",
            json!({
                "approval_id": {"type": "string", "format": "uuid"},
                "decision": {"type": "string", "enum": ["approve", "deny"]}
            }),
            &["approval_id", "decision"],
        ),
        tool_with_required(
            "approval_execute",
            "Execute one approved action by resubmitting the exact original HttpExecutionRequest. One approval permits at most one attempt.",
            json!({
                "approval_id": {"type": "string", "format": "uuid"},
                "request": http_execution_request_schema()
            }),
            &["approval_id", "request"],
        ),
        tool(
            "execution_get",
            "Get durable lifecycle evidence for one hosted execution by ID.",
            uuid_schema("execution_id"),
        ),
        tool(
            "scenario_create",
            "Create a reusable Scenario with an upstream port and inline contract definitions.",
            json!({
                "name": {"type": "string"},
                "port": {"type": "integer", "minimum": 1, "maximum": 65535},
                "observation": {
                    "type": "object",
                    "properties": {
                        "within_ms": {"type": "integer", "minimum": 0},
                        "settle_ms": {"type": "integer", "minimum": 0}
                    },
                    "additionalProperties": false
                },
                "ordering": {
                    "type": "string",
                    "enum": ["declared"]
                },
                "contracts": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "properties": {
                            "name": {"type": "string"},
                            "operation": {"type": "string"},
                            "request": {},
                            "response": {},
                            "context": {
                    "type": "object",
                    "properties": {
                        "trace_id": {"type": "string"},
                        "parent_span_id": {"type": "string"},
                        "request_id": {"type": "string"},
                        "correlation_id": {"type": "string"},
                        "causation_id": {"type": "string"},
                        "message_id": {"type": "string"},
                        "idempotency_key": {"type": "string"}
                    },
                    "additionalProperties": false
                },
                            "count": {"type": "integer", "minimum": 0},
                            "min": {"type": "integer", "minimum": 0},
                            "max": {"type": "integer", "minimum": 0}
                        },
                        "required": ["name", "operation"],
                        "additionalProperties": false
                    }
                }
            }),
        ),
        tool(
            "scenario_get",
            "Get a persisted Scenario definition.",
            uuid_schema("scenario_id"),
        ),
        tool(
            "scenario_start",
            "Start a Scenario run and create its unique Exposure.",
            uuid_schema("scenario_id"),
        ),
        tool(
            "scenario_complete",
            "Complete a Scenario run by recording its evidence, replaying it, asserting contracts, persisting the outcome, and revoking the Exposure.",
            uuid_schema("run_id"),
        ),
        tool(
            "scenario_outcome_get",
            "Get a persisted Scenario outcome by run ID.",
            uuid_schema("run_id"),
        ),
        tool(
            "interactions_list",
            "List canonical HOOKTRY interaction evidence.",
            json!({}),
        ),
        tool(
            "recording_create",
            "Snapshot current interactions into an immutable recording.",
            json!({}),
        ),
        tool(
            "recording_replay",
            "Replay an immutable recording and return replayed interaction evidence.",
            uuid_schema("recording_id"),
        ),
        tool(
            "contract_create",
            "Create a persisted contract. operation, request, response, and normalized correlation context are optional subset matchers.",
            json!({
                "name": {"type": "string"},
                "operation": {},
                "request": {},
                "response": {},
                "context": {
                    "type": "object",
                    "properties": {
                        "trace_id": {"type": "string"},
                        "parent_span_id": {"type": "string"},
                        "request_id": {"type": "string"},
                        "correlation_id": {"type": "string"},
                        "causation_id": {"type": "string"},
                        "message_id": {"type": "string"},
                        "idempotency_key": {"type": "string"}
                    },
                    "additionalProperties": false
                }
            }),
        ),
        tool(
            "contract_get",
            "Get a persisted contract by ID.",
            uuid_schema("contract_id"),
        ),
        tool(
            "contract_assert",
            "Assert one captured interaction against a persisted HOOKTRY contract.",
            json!({
                "contract_id": {"type": "string", "format": "uuid"},
                "interaction_id": {"type": "string", "format": "uuid"}
            }),
        ),
        tool(
            "assertion_get",
            "Get persisted structured assertion evidence by ID.",
            uuid_schema("assertion_id"),
        ),
    ]
}

fn tool(name: &str, description: &str, properties: Value) -> Value {
    let required = match &properties {
        Value::Object(items) => items
            .keys()
            .filter(|key| {
                *key == "name" || *key == "port" || *key == "contracts" || key.ends_with("_id")
            })
            .cloned()
            .map(Value::String)
            .collect::<Vec<_>>(),
        _ => Vec::new(),
    };

    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false
        }
    })
}

fn tool_with_required(
    name: &str,
    description: &str,
    properties: Value,
    required: &[&str],
) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": {
            "type": "object",
            "properties": properties,
            "required": required,
            "additionalProperties": false
        }
    })
}

fn http_execution_request_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "method": {"type": "string"},
            "url": {"type": "string", "format": "uri"},
            "headers": {
                "type": "object",
                "additionalProperties": {"type": "string"}
            },
            "body": {},
            "secret_headers": {
                "type": "object",
                "additionalProperties": {
                    "type": "object",
                    "properties": {
                        "secret_ref": {
                            "type": "string",
                            "pattern": "^hooktry://secrets/"
                        },
                        "prefix": {"type": "string"},
                        "suffix": {"type": "string"}
                    },
                    "required": ["secret_ref"],
                    "additionalProperties": false
                }
            },
            "capture": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "json_pointer": {"type": "string"},
                        "secret_name": {"type": "string"}
                    },
                    "required": ["json_pointer", "secret_name"],
                    "additionalProperties": false
                }
            },
            "timeout_ms": {
                "type": "integer",
                "minimum": 1,
                "maximum": 30000
            }
        },
        "required": ["method", "url"],
        "additionalProperties": false
    })
}

fn uuid_schema(name: &str) -> Value {
    json!({name: {"type": "string", "format": "uuid"}})
}

async fn call_tool(base_url: &str, hosted: &HostedClient, params: Value) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "tools/call requires a tool name".to_owned())?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let value = match name {
        "approval_inbox" => {
            return hosted_tool_result(hosted.approval_inbox().await);
        }
        "approval_create" => {
            let request = http_execution_request_argument(&arguments)?;
            return hosted_tool_result(hosted.create_approval(&request).await);
        }
        "approval_get" => {
            let id = uuid_argument(&arguments, "approval_id")?;
            return hosted_tool_result(hosted.get_approval(id).await);
        }
        "approval_decide" => {
            let id = uuid_argument(&arguments, "approval_id")?;
            let decision = approval_decision_argument(&arguments)?;
            return hosted_tool_result(hosted.decide_approval(id, decision).await);
        }
        "approval_execute" => {
            let id = uuid_argument(&arguments, "approval_id")?;
            let request = http_execution_request_argument(&arguments)?;
            return hosted_tool_result(hosted.execute_approved(id, &request).await);
        }
        "execution_get" => {
            let id = uuid_argument(&arguments, "execution_id")?;
            return hosted_tool_result(hosted.get_execution(id).await);
        }
        "scenario_create" => {
            let scenario_name = string_argument(&arguments, "name")?;
            let port = port_argument(&arguments)?;
            let contracts = arguments
                .get("contracts")
                .cloned()
                .ok_or_else(|| "contracts is required".to_owned())?;
            let observation = arguments
                .get("observation")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let ordering = arguments.get("ordering").cloned();
            api_json(
                "POST",
                &format!("{base_url}/_hooktry/scenarios"),
                Some(json!({
                    "name": scenario_name,
                    "port": port,
                    "observation": observation,
                    "ordering": ordering,
                    "contracts": contracts
                })),
            )
            .await?
        }
        "scenario_get" => {
            let id = uuid_argument(&arguments, "scenario_id")?;
            api_json("GET", &format!("{base_url}/_hooktry/scenarios/{id}"), None).await?
        }
        "scenario_start" => {
            let id = uuid_argument(&arguments, "scenario_id")?;
            api_json(
                "POST",
                &format!("{base_url}/_hooktry/scenarios/{id}/start"),
                None,
            )
            .await?
        }
        "scenario_complete" => {
            let id = uuid_argument(&arguments, "run_id")?;
            api_json(
                "POST",
                &format!("{base_url}/_hooktry/scenario-runs/{id}/complete"),
                None,
            )
            .await?
        }
        "scenario_outcome_get" => {
            let id = uuid_argument(&arguments, "run_id")?;
            api_json(
                "GET",
                &format!("{base_url}/_hooktry/scenario-runs/{id}/outcome"),
                None,
            )
            .await?
        }
        "exposure_create" => {
            let name = string_argument(&arguments, "name")?;
            let port = port_argument(&arguments)?;
            api_json(
                "POST",
                &format!("{base_url}/_hooktry/exposures"),
                Some(json!({"name": name, "port": port})),
            )
            .await?
        }
        "exposure_get" => {
            let id = uuid_argument(&arguments, "exposure_id")?;
            api_json("GET", &format!("{base_url}/_hooktry/exposures/{id}"), None).await?
        }
        "exposure_revoke" => {
            let id = uuid_argument(&arguments, "exposure_id")?;
            api_json("DELETE", &format!("{base_url}/_hooktry/exposures/{id}"), None).await?
        }
        "interactions_list" => {
            api_json("GET", &format!("{base_url}/_hooktry/interactions"), None).await?
        }
        "recording_create" => {
            api_json("POST", &format!("{base_url}/_hooktry/recordings"), None).await?
        }
        "recording_replay" => {
            let id = uuid_argument(&arguments, "recording_id")?;
            api_json(
                "POST",
                &format!("{base_url}/_hooktry/recordings/{id}/replay"),
                None,
            )
            .await?
        }
        "contract_create" => {
            let name = string_argument(&arguments, "name")?;
            let body = json!({
                "name": name,
                "operation": arguments.get("operation").cloned(),
                "request": arguments.get("request").cloned(),
                "response": arguments.get("response").cloned(),
                "context": arguments.get("context").cloned()
            });
            api_json("POST", &format!("{base_url}/_hooktry/contracts"), Some(body)).await?
        }
        "contract_get" => {
            let id = uuid_argument(&arguments, "contract_id")?;
            api_json("GET", &format!("{base_url}/_hooktry/contracts/{id}"), None).await?
        }
        "contract_assert" => {
            let contract_id = uuid_argument(&arguments, "contract_id")?;
            let interaction_id = uuid_argument(&arguments, "interaction_id")?;
            api_json(
                "POST",
                &format!("{base_url}/_hooktry/contracts/{contract_id}/assert/{interaction_id}"),
                None,
            )
            .await?
        }
        "assertion_get" => {
            let id = uuid_argument(&arguments, "assertion_id")?;
            api_json("GET", &format!("{base_url}/_hooktry/assertions/{id}"), None).await?
        }
        _ => return Ok(tool_error(format!("unknown tool: {name}"))),
    };

    tool_success(value)
}

async fn api_json(method: &str, url: &str, body: Option<Value>) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let request = match method {
        "GET" => client.get(url),
        "POST" => client.post(url),
        "DELETE" => client.delete(url),
        _ => return Err(format!("unsupported HOOKTRY API method: {method}")),
    };
    let request = if let Some(body) = body {
        request
            .header("content-type", "application/json")
            .body(serde_json::to_vec(&body).map_err(|error| error.to_string())?)
    } else {
        request
    };
    let response = request.send().await.map_err(|error| error.to_string())?;

    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Ok(tool_error(format!(
            "HOOKTRY API returned HTTP {status}: {body}"
        )));
    }

    serde_json::from_str(&body).map_err(|error| format!("invalid HOOKTRY API JSON: {error}"))
}

fn tool_success(value: Value) -> Result<Value, String> {
    Ok(json!({
        "content": [{"type": "text", "text": serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?}],
        "structuredContent": value,
        "isError": false
    }))
}

fn tool_error(message: String) -> Value {
    json!({
        "content": [{"type": "text", "text": message}],
        "isError": true
    })
}

fn hosted_tool_result(result: Result<Value, String>) -> Result<Value, String> {
    match result {
        Ok(value) => tool_success(value),
        Err(error) => Ok(tool_error(error)),
    }
}

fn http_execution_request_argument(arguments: &Value) -> Result<HttpExecutionRequest, String> {
    let request = arguments
        .get("request")
        .cloned()
        .ok_or_else(|| "request is required".to_owned())?;
    serde_json::from_value(request)
        .map_err(|error| format!("request must be a valid HttpExecutionRequest: {error}"))
}

fn approval_decision_argument(arguments: &Value) -> Result<ApprovalDecision, String> {
    match arguments.get("decision").and_then(Value::as_str) {
        Some("approve") => Ok(ApprovalDecision::Approve),
        Some("deny") => Ok(ApprovalDecision::Deny),
        _ => Err("decision must be approve or deny".to_owned()),
    }
}

fn uuid_argument(arguments: &Value, name: &str) -> Result<Uuid, String> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{name} is required"))?
        .parse()
        .map_err(|_| format!("{name} must be a UUID"))
}

fn port_argument(arguments: &Value) -> Result<u16, String> {
    let port = arguments
        .get("port")
        .and_then(Value::as_u64)
        .ok_or_else(|| "port is required".to_owned())?;
    u16::try_from(port)
        .ok()
        .filter(|port| *port > 0)
        .ok_or_else(|| "port must be between 1 and 65535".to_owned())
}

fn string_argument<'a>(arguments: &'a Value, name: &str) -> Result<&'a str, String> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{name} is required"))
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}
