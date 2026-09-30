use std::{
    collections::BTreeMap,
    net::IpAddr,
    time::{Duration, Instant},
};

use reqwest::{Method, Url, redirect::Policy};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::lookup_host;
use uuid::Uuid;

use crate::secret::{SecretRef, SecretStore};

const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_BODY_BYTES: usize = 1024 * 1024;
const MAX_TIMEOUT_MS: u64 = 30_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HttpExecutionRequest {
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub body: Option<Value>,
    #[serde(default)]
    pub secret_headers: BTreeMap<String, String>,
    #[serde(default)]
    pub capture: Vec<SecretCapture>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretCapture {
    pub json_pointer: String,
    pub secret_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionEvidence {
    pub execution_id: Uuid,
    pub status: u16,
    pub headers: BTreeMap<String, String>,
    pub body: Value,
    pub duration_ms: u64,
    pub captured_secrets: Vec<CapturedSecret>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapturedSecret {
    pub name: String,
    pub secret_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionError {
    InvalidRequest,
    UnsafeDestination,
    SecretNotFound,
    RequestFailed,
    ResponseTooLarge,
    CaptureFailed,
}

#[derive(Clone)]
pub struct HttpExecutionProvider {
    client: reqwest::Client,
    secrets: SecretStore,
}

impl HttpExecutionProvider {
    pub fn new(secrets: SecretStore) -> Self {
        let client = reqwest::Client::builder()
            .redirect(Policy::none())
            .build()
            .expect("HTTP client configuration is valid");
        Self { client, secrets }
    }

    pub async fn execute(
        &self,
        workspace_id: Uuid,
        request: HttpExecutionRequest,
    ) -> Result<ExecutionEvidence, ExecutionError> {
        self.execute_inner(workspace_id, request, true).await
    }

    async fn execute_inner(
        &self,
        workspace_id: Uuid,
        request: HttpExecutionRequest,
        enforce_public_destination: bool,
    ) -> Result<ExecutionEvidence, ExecutionError> {
        let method = Method::from_bytes(request.method.as_bytes())
            .map_err(|_| ExecutionError::InvalidRequest)?;
        let url = Url::parse(&request.url).map_err(|_| ExecutionError::InvalidRequest)?;
        if enforce_public_destination {
            validate_public_destination(&url).await?;
        }

        let body = request
            .body
            .as_ref()
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| ExecutionError::InvalidRequest)?;
        if body.as_ref().is_some_and(|body| body.len() > MAX_REQUEST_BODY_BYTES) {
            return Err(ExecutionError::InvalidRequest);
        }
        if request.timeout_ms == 0 || request.timeout_ms > MAX_TIMEOUT_MS {
            return Err(ExecutionError::InvalidRequest);
        }

        let mut builder = self
            .client
            .request(method, url)
            .timeout(Duration::from_millis(request.timeout_ms));
        for (name, value) in request.headers {
            if name.eq_ignore_ascii_case("host") || name.eq_ignore_ascii_case("content-length") {
                return Err(ExecutionError::InvalidRequest);
            }
            builder = builder.header(&name, value);
        }
        for (name, secret_name) in request.secret_headers {
            let value = self
                .secrets
                .resolve(workspace_id, &secret_name)
                .map_err(|_| ExecutionError::SecretNotFound)?;
            builder = builder.header(&name, value);
        }
        if let Some(body) = body {
            builder = builder
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body);
        }

        let started = Instant::now();
        let response = builder.send().await.map_err(|_| ExecutionError::RequestFailed)?;
        if response.status().is_redirection() {
            return Err(ExecutionError::UnsafeDestination);
        }
        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                if is_sensitive_header(name.as_str()) {
                    None
                } else {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (name.to_string(), value.to_owned()))
                }
            })
            .collect();
        let bytes = response.bytes().await.map_err(|_| ExecutionError::RequestFailed)?;
        if bytes.len() > MAX_RESPONSE_BODY_BYTES {
            return Err(ExecutionError::ResponseTooLarge);
        }
        let mut body: Value = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));

        let mut captured_secrets = Vec::new();
        for capture in request.capture {
            let value = body
                .pointer(&capture.json_pointer)
                .and_then(Value::as_str)
                .ok_or(ExecutionError::CaptureFailed)?
                .to_owned();
            let secret_ref = self
                .secrets
                .put(workspace_id, capture.secret_name.clone(), value)
                .map_err(|_| ExecutionError::CaptureFailed)?;
            redact_pointer(&mut body, &capture.json_pointer)?;
            captured_secrets.push(CapturedSecret {
                name: capture.secret_name,
                secret_ref: format!("ortyo://secrets/{}", secret_ref.name),
            });
        }

        Ok(ExecutionEvidence {
            execution_id: Uuid::now_v7(),
            status,
            headers,
            body,
            duration_ms: started.elapsed().as_millis() as u64,
            captured_secrets,
        })
    }

    #[doc(hidden)]
    pub async fn execute_for_test(
        &self,
        workspace_id: Uuid,
        request: HttpExecutionRequest,
    ) -> Result<ExecutionEvidence, ExecutionError> {
        self.execute_inner(workspace_id, request, false).await
    }

    pub fn secret_store(&self) -> &SecretStore {
        &self.secrets
    }
}

async fn validate_public_destination(url: &Url) -> Result<(), ExecutionError> {
    if url.scheme() != "https" && url.scheme() != "http" {
        return Err(ExecutionError::InvalidRequest);
    }
    let host = url.host_str().ok_or(ExecutionError::InvalidRequest)?;
    if host.eq_ignore_ascii_case("localhost") || host.ends_with(".localhost") {
        return Err(ExecutionError::UnsafeDestination);
    }
    let port = url
        .port_or_known_default()
        .ok_or(ExecutionError::InvalidRequest)?;
    let addresses = lookup_host((host, port))
        .await
        .map_err(|_| ExecutionError::RequestFailed)?
        .collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err(ExecutionError::UnsafeDestination);
    }
    Ok(())
}

fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_broadcast()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.octets()[0] == 0
                || ip.octets()[0] >= 224
                || ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1])
                || ip.octets()[0] == 169 && ip.octets()[1] == 254)
        }
        IpAddr::V6(ip) => {
            !(ip.is_loopback()
                || ip.is_unspecified()
                || ip.is_multicast()
                || ip.is_unique_local()
                || ip.is_unicast_link_local())
        }
    }
}

fn is_sensitive_header(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "authorization" | "proxy-authorization" | "cookie" | "set-cookie"
    )
}

fn redact_pointer(body: &mut Value, pointer: &str) -> Result<(), ExecutionError> {
    if pointer.is_empty() {
        *body = Value::String("[REDACTED]".to_owned());
        return Ok(());
    }
    let (parent, leaf) = pointer
        .rsplit_once('/')
        .ok_or(ExecutionError::CaptureFailed)?;
    let leaf = leaf.replace("~1", "/").replace("~0", "~");
    let parent = if parent.is_empty() {
        body
    } else {
        body.pointer_mut(parent).ok_or(ExecutionError::CaptureFailed)?
    };
    match parent {
        Value::Object(map) => {
            if !map.contains_key(&leaf) {
                return Err(ExecutionError::CaptureFailed);
            }
            map.insert(leaf, Value::String("[REDACTED]".to_owned()));
            Ok(())
        }
        Value::Array(values) => {
            let index: usize = leaf.parse().map_err(|_| ExecutionError::CaptureFailed)?;
            let value = values.get_mut(index).ok_or(ExecutionError::CaptureFailed)?;
            *value = Value::String("[REDACTED]".to_owned());
            Ok(())
        }
        _ => Err(ExecutionError::CaptureFailed),
    }
}

fn default_timeout_ms() -> u64 {
    10_000
}
