# AGENT2 - Agent-Native Approval and Execution Surface

Status: executable vertical slice  
Tracking: #116

AGENT2 projects the existing CONTROL1 + EXEC5 hosted lifecycle into MCP and CLI. It does not add a second approval model or a second execution model.

## Goal

An agent or operator should not need to construct private hosted endpoint paths manually.

The canonical lifecycle is:

```text
approval_create
    |
    v
approval_inbox (approver handoff)
    |
    v
approval_get
    |
    v
approval_decide
    |
    v
approval_execute
    |
    v
execution_get
```

These operations are projections over the existing hosted HTTP API and its durable records.

## MCP

The live `tools/list` surface includes:

- `approval_inbox`
- `approval_create`
- `approval_get`
- `approval_decide`
- `approval_execute`
- `execution_get`

The approval create/execute tools accept a typed `HttpExecutionRequest`.

The MCP schema intentionally exposes typed SecretRef bindings:

```json
{
  "secret_headers": {
    "authorization": {
      "secret_ref": "hooktry://secrets/provider-token",
      "prefix": "Bearer ",
      "suffix": ""
    }
  }
}
```

Raw credential values are not MCP arguments.

## CLI

The equivalent structured-JSON CLI is:

```text
hooktry --base-url https://... approval inbox
hooktry --base-url https://... approval create request.json
hooktry --base-url https://... approval get <approval-id>
hooktry --base-url https://... approval approve <approval-id>
hooktry --base-url https://... approval deny <approval-id>
hooktry --base-url https://... approval execute <approval-id> request.json
hooktry --base-url https://... execution get <execution-id>
```

The same request JSON file can be reused for create and execute. CONTROL1 canonical request hashing remains authoritative, so whitespace and JSON object key order do not create a different approval action, while execution-relevant values still must match exactly.

## Credential boundary

Hosted agent surfaces never accept Hooktry credentials as command-line arguments or MCP tool arguments.

The process environment provides authority:

- `HOOKTRY_TOKEN` - ask, inspect, act, and query durable execution proof
- `HOOKTRY_APPROVER_TOKEN` - approve/deny and, for an approver-only process, inspect an approval

The critical invariant is:

```text
approval_decide
    requires HOOKTRY_APPROVER_TOKEN
    never falls back to HOOKTRY_TOKEN
```

Merely discovering `approval_decide` through `tools/list` does not grant approval authority.

An operator may intentionally configure both credentials in one MCP process, but Hooktry does not silently combine them.

## Errors

Hosted HTTP errors are surfaced as MCP tool errors or CLI transport/API errors without rewriting the underlying hosted `error.code` semantics.

EXEC5 execution IDs remain queryable through `execution_get`.

## Boundary

AGENT2 does not add:

- automatic self-approval
- credential issuance/login
- raw secret resolution in agent context
- approval list/search
- execution list/search
- retries/resume
- workflow orchestration
- scheduler semantics
- VM/container lifecycle
