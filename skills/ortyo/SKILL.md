---
name: ortyo
description: Use ORTYO to expose integration boundaries, capture canonical evidence, replay interactions, and verify behavior with machine-readable scenarios and contracts.
---

# ORTYO Agent Skill

Use ORTYO as the boundary around external interactions. Do not treat it as a generic VM, tunnel, proxy, observability backend, or secret manager.

## Choose the interface

Prefer interfaces in this order:

1. MCP when `ortyo mcp` is available.
2. ORTYO CLI when shell access is available.
3. HTTP API when integrating programmatically.

For MCP, always call `initialize` and `tools/list` first. The returned tool schemas are authoritative. Do not hard-code a stale tool catalog from this file.

Successful CLI commands emit structured JSON. Preserve stdout for machine-readable ORTYO output.

## Scenario-first workflow

Use a Scenario when the goal is to verify integration behavior repeatedly.

Typical flow:

```text
scenario_create
    -> scenario_start
    -> trigger the application behavior
    -> scenario_complete
    -> scenario_outcome_get
```

The Scenario owns a unique Exposure for the run, gathers matching evidence, replays it, evaluates Contracts, persists the outcome, and revokes the Exposure.

Treat a failed ScenarioOutcome as behavioral evidence, not as an infrastructure exception. Treat transport/API/tool failures separately.

## Lower-level evidence workflow

When you need direct control:

```text
Exposure
  -> Interaction evidence
  -> Recording
  -> Replay
  -> Contract
  -> Assertion
```

Keep source and replayed interactions distinct. Replayed interactions preserve `source_interaction_id`.

Use `observed_sequence`, not UUID order or wall-clock timestamps, when reasoning about committed interaction ordering.

## Anonymous Exposure workflow

When the task only needs an inbound webhook/test endpoint, an agent may create an anonymous Exposure before authentication:

```text
POST /_ortyo/anonymous/exposures
  -> ingress_url
  -> viewer_url (WebSocket)
  -> claim_url
```

Use the ingress URL only with webhook senders. Use the viewer capability only to read backlog/live interactions. Use the claim capability only when attaching the existing Exposure and history to a Workspace. These are three different bearer capabilities and must not be interchanged or logged.

Anonymous policy is five-day hard expiry, 100 requests, 5 MiB/request, 50 MiB retained body data, and up to three active anonymous Exposures per anonymous principal. The viewer is push-only: consume the WebSocket backlog + live stream and do not poll.

## Public Exposure workflow

A public relay Exposure is controlled reachability through ORTYO. It is not itself an authentication credential.

When using a hosted Exposure:

- workspace API credentials authorize control-plane operations;
- a separate short-lived runtime capability authorizes runtime registration;
- never print or return either raw credential;
- do not persist runtime capabilities outside the intended ORTYO path.

## Hosted execution and secrets

For hosted outbound HTTP execution:

- prefer typed SecretRefs such as `ortyo://secrets/provider-token`;
- never resolve a SecretRef in agent context;
- let the executor materialize the credential at the outbound request boundary;
- respect the SecretRef's exact allowed HTTP origin;
- never attempt to bypass a wrong-origin denial with legacy syntax or direct storage access.

Captured secret values must appear as `[REDACTED]` in Evidence. Raw values belong only in the encrypted secret store and at the authorized point of use.

## Approval-gated execution

When an action requires explicit approval, prefer the agent-native sequence:

```text
approval_create
  -> approval_inbox
  -> approval_get
  -> approval_decide
  -> approval_execute
  -> execution_get
```

1. Create an approval with the exact `HttpExecutionRequest` using `ORTYO_TOKEN` / `requests:execute`.
2. An approver can discover pending handoffs with `approval_inbox`. It requires `ORTYO_APPROVER_TOKEN` and returns only pending approvals in that Workspace, oldest first.
3. Present or inspect the returned redacted summary. Do not reconstruct hidden query, body, header, or secret values from hashes.
4. Approval or denial requires the MCP/CLI process to have a distinct `ORTYO_APPROVER_TOKEN`. Discovery of `approval_decide` is not approval authority.
5. Execute by resubmitting the exact original request with `ORTYO_TOKEN`.
6. Query `execution_get` and use the durable EXEC5 record as the post-response proof.

Never modify an approved request before execution. A digest mismatch is a failed-closed condition, not a reason to bypass the gate. Never reuse a consumed approval. Retries require a new approval. Never assume `ORTYO_TOKEN` can approve: the client intentionally does not fall back when `ORTYO_APPROVER_TOKEN` is absent.

## Execution lifecycle

An ORTYO execution attempt may end as:

- `succeeded` with ExecutionEvidence;
- `rejected` with a typed policy/request error;
- `failed` with a typed provider/runtime error.

Use `execution_id` to correlate the attempted action with its Evidence and approval/audit data. Hosted execution lifecycle is durable and queryable at `GET /_ortyo/hosted/executions/{execution_id}` with `requests:execute`.

A durable `started` execution means ORTYO has no committed terminal proof. Do not describe it as still running after a disconnect or restart, and do not automatically retry it. `completed` contains the terminal EXEC4 outcome.

## Safety and sharp edges

- Do not send raw secrets in prompts, logs, CLI arguments, or generated reports.
- Do not assume a public URL is safe to call with a credential.
- Do not expose anonymous ingress/viewer/claim capabilities in logs, prompts, screenshots, or generated reports.
- Do not follow redirects around ORTYO destination policy.
- Do not replace `observed_sequence` with UUIDv7 ordering for deterministic event order.
- Do not bypass ORTYO by talking directly to a compute provider when the task is about an ORTYO Boundary.
- Do not treat a durable `started` execution as permission to retry a possibly side-effecting action.
- Do not give an ordinary agent credential `requests:approve` merely to simplify automation; keep approve and execute separable unless the operator intentionally combines them.

## Discovery

- `/llms.txt` - concise product and safety index.
- `/llms-full.txt` - self-contained projection composed from canonical ORTYO sources.
- `/skills/ortyo/SKILL.md` - this skill.
- MCP `tools/list` - authoritative live tool schemas.
