# CONTROL2 - Pending Approval Inbox

Status: executable vertical slice  
Tracking: #123

CONTROL2 adds a durable human-handoff projection over existing CONTROL1 ApprovalRecords.

It does not add a new task, queue, assignment, or workflow model.

## Contract

An approver can list the pending approvals for exactly one Workspace:

```text
GET /_hooktry/hosted/approvals
scope: requests:approve
```

The response is a JSON array of the existing redacted `ApprovalRecord` values.

Only `state=pending` records are returned.

## Ordering and bound

Inbox ordering is deterministic:

```text
requested_at_unix_ms ASC
approval_id ASC
```

The slice returns at most 100 pending approvals.

There is intentionally no arbitrary filtering, history search, assignment, priority, or pagination cursor yet.

## Authority

Inbox visibility is approver authority, not execute authority.

```text
requests:execute
    -> create / inspect known approval / execute

requests:approve
    -> inbox / inspect / approve / deny
```

An execute-only credential receives `403 forbidden` from the inbox endpoint.

Workspace isolation is enforced in the storage query itself. Pending approvals from another Workspace never enter the result set.

## Redaction

CONTROL2 does not create a second summary format.

The inbox returns the same durable `ApprovalRecord.summary` already defined by CONTROL1:

- method
- origin
- path without query
- header names without values
- secret header names without SecretRefs
- body SHA-256 instead of body contents
- capture names

Raw request query values, body values, header values, SecretRefs, and secret values are not persisted or returned.

## Agent-native surface

MCP:

```text
approval_inbox
```

CLI:

```text
HOOKTRY_APPROVER_TOKEN='hooktry_...' \
  hooktry --base-url https://relay.example approval inbox
```

The MCP process also uses `HOOKTRY_APPROVER_TOKEN`. The tool takes no credential arguments.

## Lifecycle projection

The inbox is a projection of CONTROL1 state, not a separate state machine.

```text
approval_create
    -> pending
    -> appears in inbox

approval_decide approve
    -> approved
    -> disappears from inbox

approval_decide deny
    -> denied
    -> disappears from inbox
```

Consumed approvals do not appear because they were already non-pending before consumption.

## Production dogfood

The hosted startup acceptance extends DOGFOOD3:

```text
ask
  -> require new pending approval in inbox
  -> approve
  -> require approval absent from inbox
  -> exact-request mismatch denial
  -> act
  -> durable execution query
  -> replay denial
```

The proof checks membership of the newly-created approval rather than requiring the entire inbox to be empty, so unrelated pending records do not make deployment acceptance nondeterministic.

## Notification boundary

The inbox remains the durable source of truth for human decision state.

CONTROL3 adds a transactional `approval_requested` outbox intent in the same database commit as each new pending ApprovalRecord. Telegram, Slack, email, webhook, or other providers must consume that durable intent rather than fire-and-forget from approval creation.

Provider delivery state must not become approval state. See [approval-notification-outbox.md](./approval-notification-outbox.md).

## Out of scope

- approved/denied/consumed history listing
- arbitrary search/filter DSL
- assignments
- comments
- priority
- due dates
- escalation
- multi-party/quorum decisions
- scheduler/queue semantics
- notification delivery state
- generic human task workflow
