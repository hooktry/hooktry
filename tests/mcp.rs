use ortyo::mcp::handle;
use serde_json::json;

#[tokio::test]
async fn initialize_advertises_tools_capability() {
    let response = handle(
        "http://127.0.0.1:7777",
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["serverInfo"]["name"], "ortyo");
    assert!(response["result"]["capabilities"]["tools"].is_object());
}

#[tokio::test]
async fn tools_list_exposes_complete_evidence_workflow() {
    let response = handle(
        "http://127.0.0.1:7777",
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    )
    .await
    .unwrap()
    .unwrap();

    let names = response["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec![
            "exposure_create",
            "exposure_get",
            "exposure_revoke",
            "scenario_create",
            "scenario_get",
            "scenario_start",
            "scenario_complete",
            "scenario_outcome_get",
            "interactions_list",
            "recording_create",
            "recording_replay",
            "contract_create",
            "contract_get",
            "contract_assert",
            "assertion_get",
        ]
    );
}

#[tokio::test]
async fn tool_schemas_require_identity_arguments() {
    let response = handle(
        "http://127.0.0.1:7777",
        json!({"jsonrpc":"2.0","id":3,"method":"tools/list","params":{}}),
    )
    .await
    .unwrap()
    .unwrap();

    let tools = response["result"]["tools"].as_array().unwrap();
    let exposure = tools
        .iter()
        .find(|tool| tool["name"] == "exposure_create")
        .unwrap();
    assert_eq!(exposure["inputSchema"]["required"], json!(["name", "port"]));

    let scenario = tools
        .iter()
        .find(|tool| tool["name"] == "scenario_create")
        .unwrap();
    let scenario_required = scenario["inputSchema"]["required"].as_array().unwrap();
    assert!(scenario_required.contains(&json!("name")));
    assert!(scenario_required.contains(&json!("port")));
    assert!(scenario_required.contains(&json!("contracts")));

    let replay = tools
        .iter()
        .find(|tool| tool["name"] == "recording_replay")
        .unwrap();
    assert_eq!(replay["inputSchema"]["required"], json!(["recording_id"]));

    let create = tools
        .iter()
        .find(|tool| tool["name"] == "contract_create")
        .unwrap();
    assert_eq!(create["inputSchema"]["required"], json!(["name"]));
    assert!(create["inputSchema"]["properties"]["context"]["properties"]["idempotency_key"].is_object());

    let scenario_context = &scenario["inputSchema"]["properties"]["contracts"]["items"]
        ["properties"]["context"]["properties"];
    assert!(scenario_context["correlation_id"].is_object());
    assert!(scenario_context["trace_id"].is_object());
}

#[tokio::test]
async fn notifications_do_not_emit_responses() {
    let response = handle(
        "http://127.0.0.1:7777",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    )
    .await
    .unwrap();

    assert!(response.is_none());
}

#[tokio::test]
async fn unknown_method_returns_json_rpc_error() {
    let response = handle(
        "http://127.0.0.1:7777",
        json!({"jsonrpc":"2.0","id":4,"method":"unknown"}),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(response["error"]["code"], -32601);
}
