# CONTROL2 - Pending Approval Inbox

Status: executable vertical slice  
Tracking: #123

CONTROL2 adds a durable human-handoff projection over existing CONTROL1 ApprovalRecords.

It does not add a new task, queue, assignment, or workflow model.

## Contract

An approver can list the pending approvals for exactly one Workspace:

```text
GET /_ortyo/hosted/approvals
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
ORTYO_APPROVER_TOKEN='ortyo_...' \
  ortyo --base-url https://relay.example approval inbox
```

The MCP process also uses `ORTYO_APPROVER_TOKEN`. The tool takes no credential arguments.

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

## Notification boundary

The inbox is the durable source of truth for future human notifications.

Telegram, Slack, email, webhook, or other notification providers should notify that an approval exists and point back to this durable record. Provider delivery state must not become approval state.

CONTROL2 deliberately does not add notification providers yet.

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
