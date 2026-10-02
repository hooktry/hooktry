# EXEC5 - Durable Execution Evidence

Status: executable vertical slice  
Tracking: #113

EXEC5 makes the EXEC4 lifecycle durable and queryable without turning HOOKTRY into a workflow engine.

## Contract

Before provider work starts, HOOKTRY persists a narrow lifecycle reservation:

```text
DurableExecutionRecord
  execution_id
  workspace_id
  provider
  state = started
  started_at_unix_ms
  completed_at_unix_ms = null
  outcome = null
```

When the provider attempt reaches an EXEC4 terminal outcome, the same row becomes:

```text
DurableExecutionRecord
  execution_id
  workspace_id
  provider
  state = completed
  started_at_unix_ms
  completed_at_unix_ms
  outcome
    succeeded { evidence }
    rejected { error }
    failed { error }
```

The terminal projection reconstructs the exact EXEC4 `ExecutionRecord`.

## API

Query one execution:

```text
GET /_hooktry/hosted/executions/{execution_id}
scope: requests:execute
```

Execution IDs are workspace-scoped. A credential from another Workspace receives `not_found`.

No list/search endpoint is added in this slice.

## Direct hosted execution

`POST /_hooktry/hosted/execute` now:

1. allocates an UUIDv7 execution ID
2. persists `state=started`
3. performs the existing bounded HTTP execution
4. persists the terminal EXEC4 outcome
5. returns the existing success response shape

Successful responses already contain `ExecutionEvidence.execution_id`.

For rejected/failed responses, EXEC5 additively includes the durable identity while preserving the existing error code:

```json
{
  "error": {
    "code": "unsafe_destination",
    "execution_id": "<uuid>"
  }
}
```

Clients can therefore query terminal evidence for both success and failure paths.

## Approval-gated ordering

CONTROL1 requires a stronger ordering invariant:

```text
reserve durable execution
        |
        v
consume exact approval with execution_id
        |
        v
provider attempt
        |
        v
complete durable execution
```

This means that once an ApprovalRecord is durably `consumed` and contains an `execution_id`, a durable execution reservation with that identity already exists.

If approval consumption fails because the request is pending, denied, consumed, mismatched, or belongs to another Workspace, HOOKTRY best-effort discards the unlinked `started` reservation.

## Crash semantics

`started` does **not** mean "currently running".

It means only:

> HOOKTRY durably reserved/admitted this execution identity, but no terminal proof was committed.

This distinction matters after a process crash. HOOKTRY does not guess whether the external side effect happened.

A crash after approval consumption but before terminal persistence therefore leaves useful, honest evidence:

```text
ApprovalRecord
  state = consumed
  execution_id = X

DurableExecutionRecord X
  state = started
  outcome = null
```

This is an explicit in-doubt condition, not a resumable workflow state. EXEC5 does not automatically retry it.

## Evidence boundary

The execution store does not persist the original `HttpExecutionRequest`.

It stores only:

- execution identity and Workspace ownership
- provider kind
- lifecycle timestamps
- typed terminal outcome
- successful ExecutionEvidence, whose captured secrets are already redacted by the executor

Request URLs, request body values, ordinary request headers, and SecretRef material are not copied into the lifecycle store.

## Persistence

The store uses the existing hosted persistence boundary:

- SQLite locally
- PostgreSQL in hosted production

The lifecycle transition is insert-on-reserve followed by terminal update on the same execution identity.

## Production dogfood

DOGFOOD3 is extended so the production self-proof:

1. waits until the public `/healthz.revision` matches the current `RENDER_GIT_COMMIT`
2. runs ask -> approve -> exact act
3. queries `GET /_hooktry/hosted/executions/{execution_id}`
4. requires `state=completed`
5. requires the durable terminal projection to equal the immediate `ApprovedExecution.execution`
6. then proves consumed-approval replay denial

Waiting for public revision convergence prevents Render rolling cutover from accidentally exercising an older release.

## Out of scope

EXEC5 does not add:

- execution list/search
- retries or resume
- automatic recovery of in-doubt executions
- cancellation
- queues
- scheduler semantics
- blocked/waiting workflow states
- retention or archival policy
- VM/container lifecycle
