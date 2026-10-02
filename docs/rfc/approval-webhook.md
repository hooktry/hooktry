# NOTIFY2 - Approval Webhook Provider

Status: executable vertical slice  
Tracking: #133

NOTIFY2 is the first delivery provider on top of the transactional CONTROL3 approval notification outbox.

It does not create a second approval state machine.

## Configuration

Hosted Hooktry may receive:

```text
HOOKTRY_BOOTSTRAP_WORKSPACE=<workspace slug>
HOOKTRY_APPROVAL_WEBHOOK_URL=<webhook URL>
```

The webhook URL is treated as secret material because real webhook URLs commonly contain credentials in their path or query.

On startup Hooktry:

1. validates the URL as HTTP/HTTPS with a host and no userinfo/fragment;
2. encrypts the complete URL into the existing `SecretStore`;
3. stores it under the internal name `approval-webhook-url`;
4. never logs or returns the raw URL.

The environment variable currently configures the bootstrap Workspace only. This avoids inventing a generic destination-management model before a real multi-tenant configuration surface is needed.

## Delivery payload

The webhook receives only redacted CONTROL1 information:

```json
{
  "event": "approval_requested",
  "notification_id": "<uuidv7>",
  "approval_id": "<uuidv7>",
  "requested_at_unix_ms": 0,
  "summary": {
    "method": "POST",
    "origin": "https://api.example.com",
    "path": "/danger",
    "header_names": ["x-request-id"],
    "secret_header_names": ["authorization"],
    "body_sha256": "...",
    "capture_names": []
  }
}
```

It never receives the original request body, query values, header values, SecretRefs, or secret values.

Headers include:

```text
Idempotency-Key: <notification_id>
X-Hooktry-Event: approval_requested
```

## Outbound safety

The provider does not create a second HTTP stack.

Webhook sends use the existing `HttpExecutionProvider`, so EXEC6 public-destination validation and DNS pinning apply to notification delivery too.

Redirects remain disabled.

## Durable claim / lease

Before a network send, the worker obtains a durable claim:

```text
pending outbox row
   |
   v
claim_token + lease_expires_at
   |
   v
network send
   |
   +--> 2xx -> delivered_at
   |
   +--> failure -> release claim + next_attempt_at
```

This prevents old/new Render instances overlapping during deploy from concurrently claiming the same row.

A process crash does not strand the notification: after the lease expires another worker may claim it.

The receiver should still deduplicate by `Idempotency-Key`, because a crash after the remote side accepts the request but before Hooktry commits `delivered_at` is inherently an at-least-once delivery case.

## Retry

Failures remain retryable with bounded exponential delay:

```text
5s -> 10s -> 20s -> 40s -> 80s -> 160s -> 300s cap
```

Only a short machine-readable error code is persisted as delivery metadata. Response bodies and webhook URLs are not persisted in delivery state.

## Decision race

Approval decision and stale-notification cancellation share the approval decision database transaction.

If an approval becomes approved or denied before its notification was delivered:

```text
pending approval
  + pending notification
       |
       v
approve / deny transaction
  + approval state changes
  + undelivered notification is cancelled
  + active lease is cleared
```

Already-delivered notifications remain delivered.

A worker that raced with the decision reloads the ApprovalRecord before send and does not intentionally send a non-pending approval.

## Runtime

When `HOOKTRY_APPROVAL_WEBHOOK_URL` is configured, hosted Hooktry starts one lightweight outbox worker for the bootstrap Workspace.

The worker is delivery plumbing, not a workflow scheduler. Approval state remains in CONTROL1/CONTROL2.

## Production dogfood

When the webhook provider is configured, hosted startup also proves the real external delivery path:

```text
create harmless pending approval
    -> wait for its durable outbox row to become delivered
    -> deny the still-pending approval for cleanup
    -> emit dogfood_approval_webhook_ready
```

This uses a separate approval from the normal ask-approve-act dogfood. The normal control proof intentionally decides too quickly for a pending-approval notification and therefore correctly cancels that notification before send.

The production proof logs only approval ID, notification ID, and deployed revision. It never logs the webhook URL.

## Out of scope

- generic NotificationDestination ontology
- Slack-specific formatting
- Telegram-specific formatting
- email
- assignments / escalation
- multi-party approval
- notification preferences
- user-facing secret management
- notification history UI
