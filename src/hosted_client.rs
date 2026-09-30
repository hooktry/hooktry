use reqwest::Method;
use serde_json::Value;
use uuid::Uuid;

use crate::{approval::ApprovalDecision, execution::HttpExecutionRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CredentialKind {
    Execute,
    Approve,
}

#[derive(Clone)]
pub struct HostedClient {
    base_url: String,
    http: reqwest::Client,
    execute_token: Option<String>,
    approver_token: Option<String>,
}

impl HostedClient {
    pub fn from_env(base_url: impl Into<String>) -> Self {
        Self::from_lookup(base_url, |key| std::env::var(key).ok())
    }

    pub fn from_lookup(
        base_url: impl Into<String>,
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::new(),
            execute_token: lookup("ORTYO_TOKEN").filter(|value| !value.trim().is_empty()),
            approver_token: lookup("ORTYO_APPROVER_TOKEN").filter(|value| !value.trim().is_empty()),
        }
    }

    pub async fn create_approval(&self, request: &HttpExecutionRequest) -> Result<Value, String> {
        self.request_json(
            Method::POST,
            "/_ortyo/hosted/approvals",
            CredentialKind::Execute,
            Some(serde_json::to_value(request).map_err(|error| error.to_string())?),
        )
        .await
    }

    pub async fn approval_inbox(&self) -> Result<Value, String> {
        self.request_json(
            Method::GET,
            "/_ortyo/hosted/approvals",
            CredentialKind::Approve,
            None,
        )
        .await
    }

    pub async fn get_approval(&self, approval_id: Uuid) -> Result<Value, String> {
        if self.execute_token.is_none() && self.approver_token.is_none() {
            return Err(
                "ORTYO_TOKEN or ORTYO_APPROVER_TOKEN is required to inspect approvals".to_owned(),
            );
        }
        let credential = if self.execute_token.is_some() {
            CredentialKind::Execute
        } else {
            CredentialKind::Approve
        };
        self.request_json(
            Method::GET,
            &format!("/_ortyo/hosted/approvals/{approval_id}"),
            credential,
            None,
        )
        .await
    }

    pub async fn decide_approval(
        &self,
        approval_id: Uuid,
        decision: ApprovalDecision,
    ) -> Result<Value, String> {
        self.request_json(
            Method::POST,
            &format!("/_ortyo/hosted/approvals/{approval_id}/decision"),
            CredentialKind::Approve,
            Some(serde_json::json!({"decision": decision})),
        )
        .await
    }

    pub async fn execute_approved(
        &self,
        approval_id: Uuid,
        request: &HttpExecutionRequest,
    ) -> Result<Value, String> {
        self.request_json(
            Method::POST,
            &format!("/_ortyo/hosted/approvals/{approval_id}/execute"),
            CredentialKind::Execute,
            Some(serde_json::to_value(request).map_err(|error| error.to_string())?),
        )
        .await
    }

    pub async fn get_execution(&self, execution_id: Uuid) -> Result<Value, String> {
        self.request_json(
            Method::GET,
            &format!("/_ortyo/hosted/executions/{execution_id}"),
            CredentialKind::Execute,
            None,
        )
        .await
    }

    async fn request_json(
        &self,
        method: Method,
        path: &str,
        credential: CredentialKind,
        body: Option<Value>,
    ) -> Result<Value, String> {
        let token = match credential {
            CredentialKind::Execute => self.execute_token.as_deref().ok_or_else(|| {
                "ORTYO_TOKEN is required for hosted execution operations".to_owned()
            })?,
            CredentialKind::Approve => self.approver_token.as_deref().ok_or_else(|| {
                "ORTYO_APPROVER_TOKEN is required for approval decisions".to_owned()
            })?,
        };

        let url = format!("{}{}", self.base_url, path);
        let mut request = self.http.request(method, url).bearer_auth(token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.map_err(|error| error.to_string())?;
        let status = response.status();
        let body = response.text().await.map_err(|error| error.to_string())?;

        if !status.is_success() {
            return Err(format!("ORTYO hosted API returned HTTP {status}: {body}"));
        }

        serde_json::from_str(&body)
            .map_err(|error| format!("invalid ORTYO hosted API JSON: {error}"))
    }
}
