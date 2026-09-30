# EXEC4 - Execution Lifecycle Envelope

Status: executable vertical slice  
Tracking: #104

ORTYO needs an identity for an attempted action even when the action is rejected before network work or fails before Evidence can be produced.

EXEC4 introduced a lightweight lifecycle envelope around the hosted HTTP executor. EXEC5 now persists that envelope before provider work and completes it with the terminal outcome. See [durable-execution-evidence.md](./durable-execution-evidence.md).

## Contract

```text
ExecutionRecord
  execution_id
  workspace_id
  provider = http
  started_at_unix_ms
  completed_at_unix_ms
  outcome
    succeeded
      evidence
    rejected
      error
    failed
      error
```

The `execution_id` is allocated before request validation, destination policy checks, secret resolution, or network work. The record retains the existing `workspace_id` boundary so a future durable/query layer can enforce tenant ownership without reconstructing it from external context.

For successful execution:

```text
ExecutionRecord.execution_id
        ==
ExecutionEvidence.execution_id
```

This allows the same action identity to survive across policy, execution, Evidence, and future approval/audit layers.

## Outcome semantics

`rejected` means ORTYO did not accept the attempted action under the current request/security contract.

Current examples:

- `invalid_request`
- `unsafe_destination`
- `secret_destination_denied`
- `secret_not_found`

`failed` means the action passed admission far enough to attempt provider/runtime work but did not produce valid successful Evidence.

Current examples:

- `request_failed`
- `response_too_large`
- `capture_failed`

`succeeded` contains the existing `ExecutionEvidence`.

These categories are lifecycle semantics, not HTTP status mappings.

## API compatibility

The existing:

```rust
execute(...) -> Result<ExecutionEvidence, ExecutionError>
```

remains supported.

The canonical lifecycle primitive is:

```rust
execute_recorded(...) -> ExecutionRecord
```

`execute()` is now a compatibility projection over `execute_recorded()`.

The hosted `POST /_ortyo/hosted/execute` response contract is intentionally unchanged in this pass. A future persistence/query or agent-control API can expose lifecycle records without forcing an incompatible change into the existing endpoint.

## Boundary

EXEC4 does not add:

- resumable execution
- blocked/waiting states
- workflow orchestration
- environment leases
- VM/container lifecycle
- scheduler semantics

Those are separate product decisions.

The purpose of EXEC4 is narrower: every attempted hosted action has a stable identity, timing envelope, provider kind, and typed terminal outcome.

## Why this belongs in ORTYO

This is not imported compute-provider ontology.

The lifecycle envelope describes an ORTYO Boundary action itself:

```text
Request
  -> policy / validation
  -> provider interaction
  -> Evidence or typed failure
```

Future ask-approve-act-prove can correlate approval, action, and proof using the same `execution_id` without requiring ORTYO to become a VM or workflow provider.
