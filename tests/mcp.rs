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
async fn tools_list_exposes_interactions_and_contract_assert() {
    let response = handle(
        "http://127.0.0.1:7777",
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    )
    .await
    .unwrap()
    .unwrap();

    let tools = response["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["name"], "interactions_list");
    assert_eq!(tools[1]["name"], "contract_assert");
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
        json!({"jsonrpc":"2.0","id":3,"method":"unknown"}),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(response["error"]["code"], -32601);
}
