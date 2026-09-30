use std::fmt;

use reqwest::Method;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::{
    approval::ApprovalDecision,
    execution::HttpExecutionRequest,
};

const DEFAULT_HOSTED_URL: &str = "https://ortyo.onrender.com";

#[derive(Debug, Clone, PartialEq)]
pub struct HostedApiFailure {
    pub status: u16,
    pub body: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum HostedClientError {
    Configuration(String),
    Transport(String),
    InvalidResponse(String),
    Api(HostedApiFailure),
}

impl fmt::Display for HostedClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(message)
            | Self::Transport(message)
            | Self::InvalidResponse(message) => formatter.write_str(message),
            Self::Api(failure) => write!(
                formatter,
                "HTTP {}: {}",
                failure.status,
                serde_json::to_string(&failure.body).unwrap_or_else(|_| "<invalid JSON>".to_owned())
            ),
        }
    }
}

impl std::error::Error for HostedClientError {}

#[derive(Clone)]
pub struct HostedClient {
    base_url: String,
    requester_token: Option<String>,
    approver_token: Option<String>,
    client: reqwest::Client,
}

impl HostedClient {
    pub fn from_env() -> Self {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(mut lookup: impl FnMut(&str) -> Option<String>) -> Self {
        let base_url = lookup("ORTYO_HOSTED_URL")
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_HOSTED_URL.to_owned())
            .trim_end_matches('/')
            .to_owned();
        let requester_token = lookup("ORTYO_TOKEN").filter(|value| !value.trim().is_empty());
        let approver_token =
            lookup("ORTYO_APPROVER_TOKEN").filter(|value| !value.trim().is_empty());

        Self {
            base_url,
            requester_token,
            approver_token,
            client: reqwest::Client::new(),
        }
    }

    #[doc(hidden)]
    pub fn for_test(
        base_url: impl Into<String>,
        requester_token: Option<String>,
        approver_token: Option<String>,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_owned(),
            requester_token,
            approver_token,
            client: reqwest::Client::new(),
        }
    }

    pub async fn approval_create(
        &self,
        request: &HttpExecutionRequest,
    ) -> Result<Value, HostedClientError> {
        self.request(
            Method::POST,
            "/_ortyo/hosted/approvals",
            AuthKind::Requester,
            Some(serde_json::to_value(request).map_err(invalid_request_json)?),
        )
        .await
    }

    pub async fn approval_get(&self, approval_id: Uuid) -> Result<Value, HostedClientError> {
        self.request(
            Method::GET,
            &format!("/_ortyo/hosted/approvals/{approval_id}"),
            AuthKind::Requester,
            None,
        )
        .await
    }

    pub async fn approval_decide(
        &self,
        approval_id: Uuid,
        decision: ApprovalDecision,
    ) -> Result<Value, HostedClientError> {
        self.request(
            Method::POST,
            &format!("/_ortyo/hosted/approvals/{approval_id}/decision"),
            AuthKind::Approver,
            Some(json!({"decision": decision})),
        )
        .await
    }

    pub async fn approval_execute(
        &self,
        approval_id: Uuid,
        request: &HttpExecutionRequest,
    ) -> Result<Value, HostedClientError> {
        self.request(
            Method::POST,
            &format!("/_ortyo/hosted/approvals/{approval_id}/execute"),
            AuthKind::Requester,
            Some(serde_json::to_value(request).map_err(invalid_request_json)?),
        )
        .await
    }

    pub async fn execution_get(&self, execution_id: Uuid) -> Result<Value, HostedClientError> {
        self.request(
            Method::GET,
            &format!("/_ortyo/hosted/executions/{execution_id}"),
            AuthKind::Requester,
            None,
        )
        .await
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        auth: AuthKind,
        body: Option<Value>,
    ) -> Result<Value, HostedClientError> {
        let token = match auth {
            AuthKind::Requester => self.requester_token.as_deref().ok_or_else(|| {
                HostedClientError::Configuration(
                    "ORTYO_TOKEN is required for requester operations".to_owned(),
                )
            })?,
            AuthKind::Approver => self.approver_token.as_deref().ok_or_else(|| {
                HostedClientError::Configuration(
                    "ORTYO_APPROVER_TOKEN is required for approval decisions".to_owned(),
                )
            })?,
        };

        let mut request = self
            .client
            .request(method, format!("{}{path}", self.base_url))
            .bearer_auth(token);
        if let Some(body) = body {
            request = request.json(&body);
        }

        let response = request
            .send()
            .await
            .map_err(|error| HostedClientError::Transport(error.to_string()))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|error| HostedClientError::Transport(error.to_string()))?;
        let body = serde_json::from_str::<Value>(&text).map_err(|error| {
            HostedClientError::InvalidResponse(format!(
                "hosted ORTYO returned invalid JSON: {error}"
            ))
        })?;

        if !status.is_success() {
            return Err(HostedClientError::Api(HostedApiFailure {
                status: status.as_u16(),
                body,
            }));
        }

        Ok(body)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthKind {
    Requester,
    Approver,
}

fn invalid_request_json(error: serde_json::Error) -> HostedClientError {
    HostedClientError::InvalidResponse(format!("serialize hosted request: {error}"))
}
