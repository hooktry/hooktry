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

## Public Exposure workflow

A public Exposure is controlled reachability through ORTYO. It is not itself an authentication credential.

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

When an action requires explicit approval:

1. Create an approval with the exact `HttpExecutionRequest` using a `requests:execute` credential.
2. Present or inspect the returned redacted summary. Do not reconstruct hidden query, body, header, or secret values from hashes.
3. Decide with a distinct `requests:approve` credential.
4. Execute by resubmitting the exact original request to the approval's execute endpoint.
5. Treat the returned `ApprovedExecution` as the immediate proof tying the consumed approval to the EXEC4 lifecycle record.

Never modify an approved request before execution. A digest mismatch is a failed-closed condition, not a reason to bypass the gate. Never reuse a consumed approval. Retries require a new approval.

## Execution lifecycle

An ORTYO execution attempt may end as:

- `succeeded` with ExecutionEvidence;
- `rejected` with a typed policy/request error;
- `failed` with a typed provider/runtime error.

Use `execution_id` to correlate the attempted action with its Evidence and future approval/audit data. Do not invent resumable, blocked, or workflow states unless the API explicitly exposes them.

## Safety and sharp edges

- Do not send raw secrets in prompts, logs, CLI arguments, or generated reports.
- Do not assume a public URL is safe to call with a credential.
- Do not follow redirects around ORTYO destination policy.
- Do not replace `observed_sequence` with UUIDv7 ordering for deterministic event order.
- Do not bypass ORTYO by talking directly to a compute provider when the task is about an ORTYO Boundary.
- Do not assume persistence/query APIs exist for ExecutionRecord merely because the lifecycle envelope exists.
- Do not give an ordinary agent credential `requests:approve` merely to simplify automation; keep approve and execute separable unless the operator intentionally combines them.

## Discovery

- `/llms.txt` - concise product and safety index.
- `/llms-full.txt` - self-contained projection composed from canonical ORTYO sources.
- `/skills/ortyo/SKILL.md` - this skill.
- MCP `tools/list` - authoritative live tool schemas.
