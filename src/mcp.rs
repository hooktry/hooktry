use std::io::{BufRead, Write};

use serde_json::{Value, json};
use uuid::Uuid;

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
            "serverInfo": {"name": "ortyo", "version": env!("CARGO_PKG_VERSION")}
        }),
        "tools/list" => json!({"tools": tools()}),
        "tools/call" => {
            call_tool(
                base_url,
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
            "interactions_list",
            "List canonical ORTYO interaction evidence.",
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
            "Create a persisted contract. operation, request, and response are optional subset matchers.",
            json!({
                "name": {"type": "string"},
                "operation": {},
                "request": {},
                "response": {}
            }),
        ),
        tool(
            "contract_get",
            "Get a persisted contract by ID.",
            uuid_schema("contract_id"),
        ),
        tool(
            "contract_assert",
            "Assert one captured interaction against a persisted ORTYO contract.",
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
            .filter(|key| *key == "name" || key.ends_with("_id"))
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

fn uuid_schema(name: &str) -> Value {
    json!({name: {"type": "string", "format": "uuid"}})
}

async fn call_tool(base_url: &str, params: Value) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "tools/call requires a tool name".to_owned())?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let value = match name {
        "interactions_list" => {
            api_json("GET", &format!("{base_url}/_ortyo/interactions"), None).await?
        }
        "recording_create" => {
            api_json("POST", &format!("{base_url}/_ortyo/recordings"), None).await?
        }
        "recording_replay" => {
            let id = uuid_argument(&arguments, "recording_id")?;
            api_json(
                "POST",
                &format!("{base_url}/_ortyo/recordings/{id}/replay"),
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
                "response": arguments.get("response").cloned()
            });
            api_json("POST", &format!("{base_url}/_ortyo/contracts"), Some(body)).await?
        }
        "contract_get" => {
            let id = uuid_argument(&arguments, "contract_id")?;
            api_json("GET", &format!("{base_url}/_ortyo/contracts/{id}"), None).await?
        }
        "contract_assert" => {
            let contract_id = uuid_argument(&arguments, "contract_id")?;
            let interaction_id = uuid_argument(&arguments, "interaction_id")?;
            api_json(
                "POST",
                &format!("{base_url}/_ortyo/contracts/{contract_id}/assert/{interaction_id}"),
                None,
            )
            .await?
        }
        "assertion_get" => {
            let id = uuid_argument(&arguments, "assertion_id")?;
            api_json("GET", &format!("{base_url}/_ortyo/assertions/{id}"), None).await?
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
        _ => return Err(format!("unsupported ORTYO API method: {method}")),
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
            "ORTYO API returned HTTP {status}: {body}"
        )));
    }

    serde_json::from_str(&body).map_err(|error| format!("invalid ORTYO API JSON: {error}"))
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

fn uuid_argument(arguments: &Value, name: &str) -> Result<Uuid, String> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{name} is required"))?
        .parse()
        .map_err(|_| format!("{name} must be a UUID"))
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
