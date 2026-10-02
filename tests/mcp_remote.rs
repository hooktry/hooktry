use axum::body::Body;
use hooktry::{
    hosted::{HostedRelayState, hosted_relay_app},
    mcp_remote,
    relay::RelayBroker,
    relay_auth::CapabilityStore,
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tower::ServiceExt;

fn state(public_base_url: &str) -> HostedRelayState {
    HostedRelayState::websocket_only(
        RelayBroker::default(),
        CapabilityStore::default(),
        public_base_url,
        "test-control-token",
    )
}

#[tokio::test]
async fn remote_discovery_advertises_modern_protocol_without_events_yet() {
    let response = mcp_remote::handle(
        &state("https://hooktry.example"),
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "server/discover",
            "params": {}
        }),
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        response["result"]["supportedVersions"],
        json!(["2026-07-28"])
    );
    assert!(response["result"]["capabilities"]["tools"].is_object());
    assert!(response["result"]["capabilities"]["events"].is_null());
}

#[tokio::test]
async fn remote_tool_catalog_is_curated_and_reviewable() {
    let response = mcp_remote::handle(
        &state("https://hooktry.example"),
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    )
    .await
    .unwrap()
    .unwrap();

    let tools = response["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);

    let tool = &tools[0];
    assert_eq!(tool["name"], "create_webhook_endpoint");
    assert!(tool["title"].is_string());
    assert!(tool["description"].is_string());
    assert!(tool["inputSchema"].is_object());
    assert!(tool["outputSchema"].is_object());
    assert_eq!(tool["annotations"]["readOnlyHint"], false);
    assert_eq!(tool["annotations"]["destructiveHint"], false);
    assert_eq!(tool["annotations"]["openWorldHint"], false);
    assert_eq!(tool["annotations"]["idempotentHint"], false);
    assert_eq!(tool["securitySchemes"], json!([{"type": "noauth"}]));
}

#[tokio::test]
async fn remote_webhook_tool_returns_send_and_view_capabilities_only() {
    let response = mcp_remote::handle(
        &state("https://hooktry.example"),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "create_webhook_endpoint",
                "arguments": {}
            }
        }),
    )
    .await
    .unwrap()
    .unwrap();

    let result = &response["result"];
    assert_eq!(result["isError"], false);
    assert!(
        result["structuredContent"]["hook_url"]
            .as_str()
            .unwrap()
            .starts_with("https://hooktry.example/hook/hk_")
    );

    assert!(
        result["structuredContent"]["view_url"]
            .as_str()
            .unwrap()
            .starts_with("https://hooktry.example/view/vw_")
    );

    let serialized = serde_json::to_string(result).unwrap();
    for forbidden in [
        "view_websocket_url",
        "claim_url",
        "anonymous_principal",
        "cl_",
    ] {
        assert!(
            !serialized.contains(forbidden),
            "remote result leaked forbidden capability marker: {forbidden}"
        );
    }
}

#[tokio::test]
async fn hosted_mcp_route_creates_a_webhook_that_accepts_traffic() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let base_url = format!("http://{addr}");
    let app = hosted_relay_app(state(&base_url));

    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::new();
    let response = client
        .post(format!("{base_url}/mcp"))
        .json(&json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "create_webhook_endpoint",
                "arguments": {}
            }
        }))
        .send()
        .await
        .unwrap();

    assert!(response.status().is_success());
    assert_eq!(
        response
            .headers()
            .get("access-control-allow-origin")
            .unwrap()
            .to_str()
            .unwrap(),
        "*"
    );

    let body: Value = response.json().await.unwrap();
    let hook_url = body["result"]["structuredContent"]["hook_url"]
        .as_str()
        .unwrap();
    let view_url = body["result"]["structuredContent"]["view_url"]
        .as_str()
        .unwrap();

    let ingress = client
        .post(hook_url)
        .header("x-hooktry-test", "mcp")
        .body(r#"{"event":"plugin.test"}"#)
        .send()
        .await
        .unwrap();
    assert!(ingress.status().is_success());

    let viewer = client
        .get(view_url)
        .header("accept", "text/html")
        .send()
        .await
        .unwrap();
    assert!(viewer.status().is_success());

    server.abort();
}

#[tokio::test]
async fn hosted_mcp_notifications_return_accepted_without_json_rpc_body() {
    let app = hosted_relay_app(state("https://hooktry.example"));
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/mcp")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "jsonrpc": "2.0",
                "method": "notifications/initialized"
            })
            .to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), axum::http::StatusCode::ACCEPTED);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(bytes.is_empty());
}
