# CONTROL1 - One-Shot Approval Gate for Hosted Execution

Status: executable vertical slice  
Tracking: #108

ORTYO can now separate asking for an action, deciding whether it may run, executing that exact action, and returning execution proof.

```text
Agent credential: requests:execute
        |
        | ask with HttpExecutionRequest
        v
ApprovalRecord(state=pending)
        |
        | inspect redacted summary
        v
Approver credential: requests:approve
        |
        | approve / deny
        v
ApprovalRecord(state=approved|denied)
        |
        | exact request resubmitted
        v
atomic approved -> consumed
        |
        v
ExecutionRecord
        |
        v
ApprovedExecution { approval, execution }
```

## Why this belongs in ORTYO

CONTROL1 governs an ORTYO action at the Boundary. It does not model VM ownership, job scheduling, workflow DAGs, or a generic human-task system.

The approval is a narrow capability for one exact hosted HTTP execution attempt.

## API

Create an approval request:

```text
POST /_ortyo/hosted/approvals
scope: requests:execute
body: HttpExecutionRequest
```

Inspect an approval:

```text
GET /_ortyo/hosted/approvals/{approval_id}
scope: requests:execute OR requests:approve
```

Decide:

```text
POST /_ortyo/hosted/approvals/{approval_id}/decision
scope: requests:approve

{"decision":"approve"}
{"decision":"deny"}
```

Execute an approved action:

```text
POST /_ortyo/hosted/approvals/{approval_id}/execute
scope: requests:execute
body: the exact HttpExecutionRequest that was approved
```

The execution endpoint returns an `ApprovedExecution` containing the consumed `ApprovalRecord` and the full EXEC4 `ExecutionRecord`. The consumed approval persists the same `execution_id` for durable correlation.

## Exact-action binding

Approval is bound to a canonical SHA-256 digest of the complete `HttpExecutionRequest`.

Canonicalization recursively sorts JSON object keys before hashing. This avoids meaningless digest changes caused only by object key insertion order.

Changing any execution-relevant input changes the digest, including:

- method
- URL or query
- ordinary headers or values
- request body
- SecretRef bindings
- capture instructions
- timeout

A mismatch fails closed with `approval_request_mismatch`. The approval remains approved and unconsumed after a mismatch.

## Redacted approval summary

The durable approval record does not store the original request.

It stores only:

- HTTP method
- exact origin
- path without query
- ordinary header names, not values
- secret header names, not SecretRefs or secret values
- canonical body SHA-256, not body contents
- capture target names
- full canonical request digest

The approval database therefore does not persist raw request body values, query values, header values, or secret material.

## One-shot semantics

An approval has these terminal/lifecycle states:

```text
pending -> approved -> consumed
       \-> denied
```

Only `pending` can be decided.

Only `approved` can be consumed.

The act endpoint allocates an EXEC4 `execution_id` before consumption. Consumption atomically stores that `execution_id` while transitioning `approved -> consumed`, then the executor uses the same identity for the attempt.

Therefore one approval permits at most one execution attempt, even when the execution later returns `rejected` or `failed`. If the process crashes after consumption but before the provider completes, the durable approval still retains the reserved execution identity rather than becoming an uncorrelated consumed grant.

This is intentional: retrying an external side effect requires a new approval.

## Separation of duties

`requests:execute` and `requests:approve` are distinct API scopes.

A normal agent credential can ask, inspect, and act without being able to approve.

An approver credential can inspect the redacted summary and approve or deny without being able to execute.

The bootstrap operator credential includes both scopes. Existing bootstrap operator credentials are upgraded server-side during startup using the encrypted SecretRef value; the raw token is never returned or logged.

## Workspace isolation

Approval IDs are workspace-scoped. Cross-workspace inspection, decision, or consumption behaves as not found.

## Persistence

ApprovalRecord state is durable in the same hosted SQLite/Postgres persistence boundary as other hosted control-plane state.

CONTROL1 does not persist EXEC4 ExecutionRecords yet. The act response carries the ExecutionRecord as proof. Durable execution query storage remains a separate decision.

## Out of scope

CONTROL1 does not add:

- workflow DAGs
- blocked/resume orchestration
- scheduled approval jobs
- multi-party or quorum approval
- wildcard approval policies
- approval templates
- environment or VM leases
- automatic retries
- durable ExecutionRecord query APIs
