# AGENT2: agent-native approval and execution lifecycle

DWC: PROD/AGENT2.1 AGENT2 - MCP + CLI adapters for CONTROL1 and EXEC5

## Decision

Expose the already-proven hosted approval and durable execution lifecycle through MCP and CLI adapters. Do not add a second domain model, a workflow engine, or new hosted endpoints.

Both adapters share one `HostedClient`.

## Surface

MCP:

- `approval_create`
- `approval_get`
- `approval_decide`
- `approval_execute`
- `execution_get`

CLI:

- `ortyo approval create FILE`
- `ortyo approval get ID`
- `ortyo approval approve ID`
- `ortyo approval deny ID`
- `ortyo approval execute ID FILE`
- `ortyo execution get ID`

`FILE` is a JSON `HttpExecutionRequest`. Create and execute intentionally use the same typed request contract so CONTROL1 can enforce exact-action approval.

## Authentication

- `ORTYO_TOKEN` is read from the process environment for requester operations.
- `ORTYO_APPROVER_TOKEN` is read from the process environment only for approval decisions.
- No token is accepted in an MCP tool input or CLI argument.
- Missing approver credentials never fall back to `ORTYO_TOKEN`.
- Missing requester credentials never fall back to `ORTYO_APPROVER_TOKEN`.

## Secret handling

Typed `SecretRef` bindings pass through as references. CLI and MCP never resolve them. Resolution remains inside the trusted hosted executor and still enforces the exact allowed HTTP origin.

## Error semantics

The hosted HTTP API remains authoritative. MCP preserves structured hosted error bodies as `structuredContent` with `isError=true`; CLI prints the existing HTTP status/error JSON and exits through the normal CLI error path.

## Non-goals

- automatic self-approval
- token/login management
- approval/execution search
- workflow retries/resume
- new hosted endpoints
- VM or environment orchestration
