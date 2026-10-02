# CONTROL3 - Transactional Approval Notification Outbox

Status: executable vertical slice  
Tracking: #128

CONTROL3 adds one durable notification intent for every newly-created pending CONTROL1 approval.

The outbox exists to close one specific crash window:

```text
persist ApprovalRecord
    |
    | process crashes here
    v
enqueue notification
```

That sequence is not acceptable because a durable approval could exist without any durable handoff intent.

CONTROL3 instead commits both rows in one database transaction.

## Transactional invariant

For every successful `approval_create`:

```text
BEGIN
  INSERT hosted_approvals
  INSERT hosted_approval_notification_outbox
COMMIT
```

If either insert fails, neither record commits.

The outbox row contains only:

- `notification_id` - UUIDv7
- `workspace_id`
- `approval_id`
- `event = approval_requested`
- `created_at_unix_ms`
- optional `delivered_at_unix_ms`

The outbox does not persist:

- request body
- query values
- ordinary header values
- SecretRefs
- secret values
- a second copy of the approval summary

Delivery code must load the existing redacted `ApprovalRecord` by `approval_id`.

## Persistence

The same semantics exist on SQLite and PostgreSQL.

Undelivered intents are workspace scoped, bounded to 100 rows, and ordered by:

```text
created_at ASC
notification_id ASC
```

Marking a notification delivered is idempotent: the first delivery timestamp is retained.

Cross-workspace delivery marking behaves as not found.

## Upgrade/backfill

On store open, CONTROL3 backfills only pre-existing `pending` approvals that do not yet have an outbox intent.

The backfill:

- creates a fresh UUIDv7 `notification_id`
- keeps the approval's original `requested_at` as the notification `created_at`
- does not copy request material
- does not create intents for approved, denied, or consumed approvals

This lets a deployment introduce the outbox without silently losing already-pending human handoffs.

## Production dogfood

Hosted startup acceptance proves the transactional invariant against the production database. Immediately after `approval_create`, HOOKTRY loads the outbox intent by `workspace_id + approval_id` and requires:

- `event = approval_requested`
- matching Workspace and Approval identities
- `created_at_unix_ms = ApprovalRecord.requested_at_unix_ms`

The proof does not require the intent to remain undelivered, because an enabled provider may legitimately race and deliver it immediately.

## Relationship to CONTROL2

CONTROL2 remains the durable human-facing source of truth:

```text
ApprovalRecord(pending)
        |
        +--> approval_inbox
        |
        +--> approval_requested outbox intent
```

The inbox answers "what currently needs a decision?"

The outbox answers "what notification delivery work still needs to happen?"

Notification delivery state is never approval state.

## Provider boundary

CONTROL3 intentionally separates durable intent from provider delivery.

NOTIFY2 is the first consumer: a leased webhook provider documented in [approval-webhook.md](./approval-webhook.md). It claims durable outbox rows, sends only the redacted ApprovalRecord projection, and marks delivery only after a 2xx response.

No provider sends directly from `create_approval`. This keeps provider outages, process restarts, retries, and deployment overlap independent from ApprovalRecord persistence.

## Out of scope

- generic event bus
- arbitrary workflow events
- notification provider configuration
- Telegram / Slack / email implementation
- retry/backoff policy
- row leasing / multi-worker claiming
- escalation
- assignments
- comments
- approval history search
