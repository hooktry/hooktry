use axum::{
    Json, Router,
    body::Bytes,
    http::StatusCode,
    routing::post,
};
use ortyo::{
    domain::{AssertionResult, Exposure, Interaction, Origin, Recording},
    http::{AppState, app},
    mcp::handle,
};
use serde_json::{Value, json};

#[tokio::test]
async fn agent_drives_exposure_evidence_replay_and_assertion_through_mcp() {
    let target = Router::new().route(
        "/webhook",
        post(|body: Bytes| async move {
            (
                StatusCode::ACCEPTED,
                Json(json!({"received": String::from_utf8_lossy(&body)})),
            )
        }),
    );
    let target_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_port = target_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(target_listener, target).await.unwrap();
    });

    let ortyo_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let ortyo_port = ortyo_listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        axum::serve(ortyo_listener, app(AppState::default()))
            .await
            .unwrap();
    });
    let base_url = format!("http://127.0.0.1:{ortyo_port}");

    let exposure_value = mcp_call(
        &base_url,
        "exposure_create",
        json!({"name": "agent-webhook", "port": target_port}),
    )
    .await;
    let exposure: Exposure = serde_json::from_value(exposure_value).unwrap();

    let response = reqwest::Client::new()
        .post(format!("{base_url}/exposed/{}/webhook?delivery=42", exposure.id))
        .header("content-type", "application/json")
        .header("x-agent-run", "mcp-run-123")
        .body(r#"{"event":"payment.created","amount":4999}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let interactions_value = mcp_call(&base_url, "interactions_list", json!({})).await;
    let interactions: Vec<Interaction> = serde_json::from_value(interactions_value).unwrap();
    assert_eq!(interactions.len(), 1);
    let source = &interactions[0];
    assert_eq!(source.origin, Origin::Proxied);
    assert_eq!(source.request["headers"]["x-agent-run"], "mcp-run-123");

    let recording_value = mcp_call(&base_url, "recording_create", json!({})).await;
    let recording: Recording = serde_json::from_value(recording_value).unwrap();
    assert_eq!(recording.interaction_ids, vec![source.id]);

    let replayed_value = mcp_call(
        &base_url,
        "recording_replay",
        json!({"recording_id": recording.id}),
    )
    .await;
    let replayed: Vec<Interaction> = serde_json::from_value(replayed_value).unwrap();
    let replay = &replayed[0];
    assert_eq!(replay.origin, Origin::Replayed);
    assert_eq!(replay.source_interaction_id, Some(source.id));

    let contract = mcp_call(
        &base_url,
        "contract_create",
        json!({
            "name": "agent webhook accepted",
            "operation": "POST /webhook",
            "request": {
                "query": "delivery=42",
                "body": "{\"event\":\"payment.created\",\"amount\":4999}"
            },
            "response": {"status": 202}
        }),
    )
    .await;
    let contract_id = contract["id"].as_str().unwrap();

    let assertion_value = mcp_call(
        &base_url,
        "contract_assert",
        json!({"contract_id": contract_id, "interaction_id": replay.id}),
    )
    .await;
    let assertion: AssertionResult = serde_json::from_value(assertion_value).unwrap();
    assert!(assertion.passed);
    assert_eq!(assertion.interaction_id, replay.id);

    let persisted_value = mcp_call(
        &base_url,
        "assertion_get",
        json!({"assertion_id": assertion.id}),
    )
    .await;
    let persisted: AssertionResult = serde_json::from_value(persisted_value).unwrap();
    assert_eq!(persisted.id, assertion.id);
    assert!(persisted.passed);

    let revoked = mcp_call(
        &base_url,
        "exposure_revoke",
        json!({"exposure_id": exposure.id}),
    )
    .await;
    assert_eq!(revoked["state"], "revoked");
}

async fn mcp_call(base_url: &str, name: &str, arguments: Value) -> Value {
    let response = handle(
        base_url,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": name, "arguments": arguments}
        }),
    )
    .await
    .unwrap()
    .unwrap();

    let result = &response["result"];
    assert_eq!(result["isError"], false, "{result}");
    result["structuredContent"].clone()
}
