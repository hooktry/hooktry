mod common;

use futures_util::StreamExt;
use ortyo::{
    anonymous::AnonymousProvision,
    hosted::{HostedRelayState, hosted_relay_app},
    relay::RelayBroker,
    relay_auth::CapabilityStore,
};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn anonymous_exposure_pushes_interactions_and_can_be_claimed_without_rotating_ingress() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = HostedRelayState::websocket_only(
        RelayBroker::default(),
        CapabilityStore::default(),
        format!("http://{addr}"),
        "test-control-token",
    );

    tokio::spawn(async move {
        axum::serve(listener, hosted_relay_app(state))
            .await
            .unwrap();
    });

    let client = reqwest::Client::new();
    let create = client
        .post(format!("http://{addr}/_ortyo/anonymous/exposures"))
        .send()
        .await
        .unwrap();
    assert_eq!(create.status(), reqwest::StatusCode::CREATED);
    assert!(create.headers().get("set-cookie").is_some());

    let provision: AnonymousProvision = create.json().await.unwrap();
    assert!(provision.ingress_url.contains("/h/ortyo_in_"));
    assert!(
        provision
            .viewer_url
            .contains("/_ortyo/anonymous/view/ortyo_view_")
    );
    assert!(
        provision
            .claim_url
            .contains("/_ortyo/anonymous/claim/ortyo_claim_")
    );
    assert_ne!(provision.ingress_url, provision.viewer_url);
    assert_ne!(provision.ingress_url, provision.claim_url);
    assert_ne!(provision.viewer_url, provision.claim_url);

    let (mut viewer, _) = tokio_tungstenite::connect_async(&provision.viewer_url)
        .await
        .unwrap();
    let ready = viewer.next().await.unwrap().unwrap();
    let ready: serde_json::Value = match ready {
        Message::Text(text) => serde_json::from_str(text.as_str()).unwrap(),
        other => panic!("unexpected viewer ready message: {other:?}"),
    };
    assert_eq!(ready["type"], "ready");
    assert_eq!(
        ready["exposure"]["exposure_id"],
        provision.exposure.exposure_id.to_string()
    );

    let ingress = format!("{}/stripe?delivery=42", provision.ingress_url);
    let first = client
        .post(&ingress)
        .header("stripe-signature", "proof")
        .body(r#"{"type":"checkout.session.completed"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), reqwest::StatusCode::OK);

    let pushed = viewer.next().await.unwrap().unwrap();
    let pushed: serde_json::Value = match pushed {
        Message::Text(text) => serde_json::from_str(text.as_str()).unwrap(),
        other => panic!("unexpected viewer interaction message: {other:?}"),
    };
    assert_eq!(pushed["type"], "interaction");
    assert_eq!(pushed["interaction"]["sequence"], 1);
    assert_eq!(pushed["interaction"]["path"], "/stripe");
    assert_eq!(pushed["interaction"]["query"], "delivery=42");
    assert_eq!(pushed["interaction"]["body_encoding"], "utf8");
    assert_eq!(
        pushed["interaction"]["body"],
        r#"{"type":"checkout.session.completed"}"#
    );

    let credential = common::issue_full_access_token(&format!("http://{addr}")).await;
    let claimed = client
        .post(&provision.claim_url)
        .bearer_auth(&credential.token)
        .send()
        .await
        .unwrap();
    assert_eq!(claimed.status(), reqwest::StatusCode::OK);
    let claimed: serde_json::Value = claimed.json().await.unwrap();
    assert_eq!(claimed["claimed"], true);
    assert!(claimed["workspace_id"].is_string());
    assert!(claimed["expires_at_unix_seconds"].is_null());

    let second = client
        .post(&ingress)
        .body(r#"{"after":"claim"}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), reqwest::StatusCode::OK);

    let pushed = viewer.next().await.unwrap().unwrap();
    let pushed: serde_json::Value = match pushed {
        Message::Text(text) => serde_json::from_str(text.as_str()).unwrap(),
        other => panic!("unexpected viewer interaction message: {other:?}"),
    };
    assert_eq!(pushed["interaction"]["sequence"], 2);
    assert_eq!(pushed["interaction"]["body"], r#"{"after":"claim"}"#);

    let reused_claim = client
        .post(&provision.claim_url)
        .bearer_auth(&credential.token)
        .send()
        .await
        .unwrap();
    assert_eq!(reused_claim.status(), reqwest::StatusCode::GONE);
}

#[tokio::test]
async fn anonymous_viewer_capability_is_not_an_ingress_capability() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let state = HostedRelayState::websocket_only(
        RelayBroker::default(),
        CapabilityStore::default(),
        format!("http://{addr}"),
        "test-control-token",
    );

    tokio::spawn(async move {
        axum::serve(listener, hosted_relay_app(state))
            .await
            .unwrap();
    });

    let client = reqwest::Client::new();
    let provision: AnonymousProvision = client
        .post(format!("http://{addr}/_ortyo/anonymous/exposures"))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    let viewer_token = provision.viewer_url.rsplit('/').next().unwrap();
    let response = client
        .post(format!("http://{addr}/h/{viewer_token}"))
        .body("must not route")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
}
