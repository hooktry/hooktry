use std::{collections::BTreeMap, time::Duration};

use reqwest::Url;
use serde::Serialize;
use tokio::time::sleep;
use uuid::Uuid;

use crate::{
    approval::{ApprovalNotificationClaim, ApprovalState, ApprovalSummary},
    execution::{ExecutionError, HttpExecutionRequest},
    hosted::HostedRelayState,
    secret::{SecretError, SecretStore},
};

pub const APPROVAL_WEBHOOK_SECRET_NAME: &str = "approval-webhook-url";

const DELIVERY_LEASE_MS: u64 = 30_000;
const IDLE_SLEEP: Duration = Duration::from_secs(1);
const MIN_RETRY_MS: u64 = 5_000;
const MAX_RETRY_MS: u64 = 5 * 60_000;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ApprovalWebhookPayload {
    pub event: &'static str,
    pub notification_id: Uuid,
    pub approval_id: Uuid,
    pub requested_at_unix_ms: u64,
    pub summary: ApprovalSummary,
}

pub fn validate_webhook_url(value: &str) -> Result<(), String> {
    let url = Url::parse(value).map_err(|_| "approval webhook URL is invalid".to_owned())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("approval webhook URL must use http or https and include a host".to_owned());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("approval webhook URL must not contain userinfo".to_owned());
    }
    if url.fragment().is_some() {
        return Err("approval webhook URL must not contain a fragment".to_owned());
    }
    Ok(())
}

pub async fn ensure_webhook_secret(
    secrets: &SecretStore,
    workspace_id: Uuid,
    url: &str,
) -> Result<(), String> {
    validate_webhook_url(url)?;
    match secrets
        .resolve_async(workspace_id, APPROVAL_WEBHOOK_SECRET_NAME.to_owned())
        .await
    {
        Ok(existing) if existing == url => Ok(()),
        Ok(_) | Err(SecretError::NotFound) => secrets
            .put_async(
                workspace_id,
                APPROVAL_WEBHOOK_SECRET_NAME.to_owned(),
                url.to_owned(),
            )
            .await
            .map(|_| ())
            .map_err(|error| format!("persist approval webhook secret: {error:?}")),
        Err(error) => Err(format!("resolve approval webhook secret: {error:?}")),
    }
}

pub async fn run_worker(state: HostedRelayState, workspace_id: Uuid) {
    loop {
        match process_one(&state, workspace_id, false).await {
            Ok(true) => {}
            Ok(false) => sleep(IDLE_SLEEP).await,
            Err(error) => {
                eprintln!(
                    "{}",
                    serde_json::json!({
                        "event": "approval_webhook_worker_error",
                        "workspace_id": workspace_id,
                        "error": error
                    })
                );
                sleep(IDLE_SLEEP).await;
            }
        }
    }
}

async fn process_one(
    state: &HostedRelayState,
    workspace_id: Uuid,
    allow_private_destination_for_test: bool,
) -> Result<bool, String> {
    let Some(claim) = state
        .approvals
        .claim_next_notification_async(workspace_id, DELIVERY_LEASE_MS)
        .await
        .map_err(|error| format!("claim approval notification: {error:?}"))?
    else {
        return Ok(false);
    };

    let approval = state
        .approvals
        .get_async(workspace_id, claim.record.approval_id)
        .await
        .map_err(|error| format!("load approval for notification: {error:?}"))?;
    let Some(approval) = approval else {
        return release_failed_claim(state, workspace_id, &claim, "approval_not_found").await;
    };

    if approval.state != ApprovalState::Pending {
        // The decision transaction cancels the delivery lease. A worker that raced
        // with that decision must not send a stale notification.
        return Ok(true);
    }

    let url = match state
        .executor
        .secret_store()
        .resolve_async(workspace_id, APPROVAL_WEBHOOK_SECRET_NAME.to_owned())
        .await
    {
        Ok(url) => url,
        Err(SecretError::NotFound) => {
            return release_failed_claim(state, workspace_id, &claim, "webhook_not_configured")
                .await;
        }
        Err(_) => {
            return release_failed_claim(state, workspace_id, &claim, "webhook_secret_error").await;
        }
    };
    if validate_webhook_url(&url).is_err() {
        return release_failed_claim(state, workspace_id, &claim, "invalid_webhook_url").await;
    }

    let payload = ApprovalWebhookPayload {
        event: "approval_requested",
        notification_id: claim.record.notification_id,
        approval_id: approval.approval_id,
        requested_at_unix_ms: approval.requested_at_unix_ms,
        summary: approval.summary,
    };
    let mut headers = BTreeMap::new();
    headers.insert(
        "idempotency-key".to_owned(),
        claim.record.notification_id.to_string(),
    );
    headers.insert("x-ortyo-event".to_owned(), "approval_requested".to_owned());

    let request = HttpExecutionRequest {
        method: "POST".to_owned(),
        url,
        headers,
        body: Some(
            serde_json::to_value(payload)
                .map_err(|error| format!("serialize approval webhook payload: {error}"))?,
        ),
        secret_headers: BTreeMap::new(),
        capture: vec![],
        timeout_ms: 5_000,
    };

    let result = if allow_private_destination_for_test {
        state.executor.execute_for_test(workspace_id, request).await
    } else {
        state.executor.execute(workspace_id, request).await
    };

    match result {
        Ok(evidence) if (200..300).contains(&evidence.status) => {
            state
                .approvals
                .complete_notification_claim_async(
                    workspace_id,
                    claim.record.notification_id,
                    claim.claim_token,
                )
                .await
                .map_err(|error| format!("complete approval notification: {error:?}"))?;
            eprintln!(
                "{}",
                serde_json::json!({
                    "event": "approval_webhook_delivered",
                    "workspace_id": workspace_id,
                    "notification_id": claim.record.notification_id,
                    "approval_id": claim.record.approval_id,
                    "attempt": claim.attempt_count,
                    "status": evidence.status
                })
            );
            Ok(true)
        }
        Ok(evidence) => {
            release_failed_claim(
                state,
                workspace_id,
                &claim,
                &format!("http_{}", evidence.status),
            )
            .await
        }
        Err(error) => {
            release_failed_claim(state, workspace_id, &claim, execution_error_code(error)).await
        }
    }
}

async fn release_failed_claim(
    state: &HostedRelayState,
    workspace_id: Uuid,
    claim: &ApprovalNotificationClaim,
    error_code: &str,
) -> Result<bool, String> {
    state
        .approvals
        .fail_notification_claim_async(
            workspace_id,
            claim.record.notification_id,
            claim.claim_token,
            retry_delay_ms(claim.attempt_count),
            error_code.to_owned(),
        )
        .await
        .map_err(|error| format!("release failed approval notification: {error:?}"))?;
    Ok(true)
}

fn execution_error_code(error: ExecutionError) -> &'static str {
    match error {
        ExecutionError::InvalidRequest => "invalid_request",
        ExecutionError::UnsafeDestination => "unsafe_destination",
        ExecutionError::SecretDestinationDenied => "secret_destination_denied",
        ExecutionError::SecretNotFound => "secret_not_found",
        ExecutionError::RequestFailed => "request_failed",
        ExecutionError::ResponseTooLarge => "response_too_large",
        ExecutionError::CaptureFailed => "capture_failed",
    }
}

fn retry_delay_ms(attempt_count: u32) -> u64 {
    let exponent = attempt_count.saturating_sub(1).min(6);
    MIN_RETRY_MS
        .saturating_mul(1_u64 << exponent)
        .min(MAX_RETRY_MS)
}

#[doc(hidden)]
pub async fn process_one_for_test(
    state: &HostedRelayState,
    workspace_id: Uuid,
) -> Result<bool, String> {
    process_one(state, workspace_id, true).await
}

#[doc(hidden)]
pub fn retry_delay_ms_for_test(attempt_count: u32) -> u64 {
    retry_delay_ms(attempt_count)
}
