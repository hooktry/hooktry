use ortyo::{
    hosted::{HostedRelayState, hosted_relay_app},
    http::{AppState, app},
    relay::RelayBroker,
    relay_auth::CapabilityStore,
};
use tokio::net::TcpListener;

#[tokio::test]
async fn local_http_publishes_agent_discovery_surfaces() {
    let base_url = spawn_local().await;

    assert_discovery_surfaces(&base_url).await;
}

#[tokio::test]
async fn hosted_http_publishes_agent_discovery_surfaces_without_authentication() {
    let base_url = spawn_hosted().await;

    assert_discovery_surfaces(&base_url).await;
}

async fn spawn_local() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app(AppState::default()))
            .await
            .unwrap();
    });
    format!("http://{addr}")
}

async fn spawn_hosted() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = HostedRelayState::websocket_only(
        RelayBroker::default(),
        CapabilityStore::default(),
        "https://ortyo.example",
        "test-control-token",
    );
    tokio::spawn(async move {
        axum::serve(listener, hosted_relay_app(state))
            .await
            .unwrap();
    });
    format!("http://{addr}")
}

async fn assert_discovery_surfaces(base_url: &str) {
    let llms = reqwest::get(format!("{base_url}/llms.txt"))
        .await
        .unwrap();
    assert!(llms.status().is_success());
    assert_eq!(
        llms.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        "text/plain; charset=utf-8"
    );
    let llms = llms.text().await.unwrap();
    assert!(llms.contains("# ORTYO"));
    assert!(llms.contains("MCP"));
    assert!(llms.contains("/skills/ortyo/SKILL.md"));
    assert!(!llms.contains("ortyo_super_secret"));

    let full = reqwest::get(format!("{base_url}/llms-full.txt"))
        .await
        .unwrap();
    assert!(full.status().is_success());
    let full = full.text().await.unwrap();
    assert!(full.contains("# Canonical README"));
    assert!(full.contains("# ORTYO Agent Skill"));
    assert!(full.contains("A programmable integration boundary"));

    let skill = reqwest::get(format!("{base_url}/skills/ortyo/SKILL.md"))
        .await
        .unwrap();
    assert!(skill.status().is_success());
    assert_eq!(
        skill.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .unwrap()
            .to_str()
            .unwrap(),
        "text/markdown; charset=utf-8"
    );
    let skill = skill.text().await.unwrap();
    assert!(skill.contains("name: ortyo"));
    assert!(skill.contains("tools/list"));
    assert!(skill.contains("never print or return either raw credential"));
}
