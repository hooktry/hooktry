use ortyo::hosted_identity::{ApiScope, IssuedApiCredential, Workspace};

pub async fn issue_full_access_token(base_url: &str) -> IssuedApiCredential {
    let client = reqwest::Client::new();
    let workspace: Workspace = client
        .post(format!("{base_url}/_ortyo/admin/workspaces"))
        .bearer_auth("test-control-token")
        .json(&serde_json::json!({"slug": format!("test-{}", uuid::Uuid::now_v7())}))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();

    client
        .post(format!(
            "{base_url}/_ortyo/admin/workspaces/{}/credentials",
            workspace.id
        ))
        .bearer_auth("test-control-token")
        .json(&serde_json::json!({
            "name": "test",
            "scopes": [
                ApiScope::ExposuresCreate,
                ApiScope::ExposuresRead,
                ApiScope::ExposuresRevoke
            ]
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap()
}
