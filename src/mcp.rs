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
        "tools/list" => json!({
            "tools": [
                {
                    "name": "interactions_list",
                    "description": "List canonical ORTYO interaction evidence.",
                    "inputSchema": {"type": "object", "properties": {}, "additionalProperties": false}
                },
                {
                    "name": "contract_assert",
                    "description": "Assert one captured interaction against a persisted ORTYO contract.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "contract_id": {"type": "string", "format": "uuid"},
                            "interaction_id": {"type": "string", "format": "uuid"}
                        },
                        "required": ["contract_id", "interaction_id"],
                        "additionalProperties": false
                    }
                }
            ]
        }),
        "tools/call" => call_tool(base_url, request.get("params").cloned().unwrap_or(Value::Null)).await?,
        _ => return Ok(Some(error(id, -32601, "method not found"))),
    };

    Ok(Some(json!({"jsonrpc": "2.0", "id": id, "result": result})))
}

async fn call_tool(base_url: &str, params: Value) -> Result<Value, String> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| "tools/call requires a tool name".to_owned())?;
    let arguments = params.get("arguments").cloned().unwrap_or_else(|| json!({}));

    let value = match name {
        "interactions_list" => fetch_json(&format!("{base_url}/_ortyo/interactions"), false).await?,
        "contract_assert" => {
            let contract_id = uuid_argument(&arguments, "contract_id")?;
            let interaction_id = uuid_argument(&arguments, "interaction_id")?;
            fetch_json(
                &format!(
                    "{base_url}/_ortyo/contracts/{contract_id}/assert/{interaction_id}"
                ),
                true,
            )
            .await?
        }
        _ => {
            return Ok(json!({
                "content": [{"type": "text", "text": format!("unknown tool: {name}")}],
                "isError": true
            }));
        }
    };

    Ok(json!({
        "content": [{"type": "text", "text": serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?}],
        "structuredContent": value,
        "isError": false
    }))
}

async fn fetch_json(url: &str, post: bool) -> Result<Value, String> {
    let client = reqwest::Client::new();
    let response = if post {
        client.post(url).send().await
    } else {
        client.get(url).send().await
    }
    .map_err(|error| error.to_string())?;

    let status = response.status();
    let body = response.text().await.map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(format!("ORTYO API returned HTTP {status}: {body}"));
    }
    serde_json::from_str(&body).map_err(|error| format!("invalid ORTYO API JSON: {error}"))
}

fn uuid_argument(arguments: &Value, name: &str) -> Result<Uuid, String> {
    arguments
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{name} is required"))?
        .parse()
        .map_err(|_| format!("{name} must be a UUID"))
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}
