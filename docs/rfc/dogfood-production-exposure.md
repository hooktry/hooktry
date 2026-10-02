# DOGFOOD1 - Production Exposure bootstrap

Status: executable vertical slice  
Tracking: #89

DOGFOOD1 proves that HOOKTRY can use its own durable bootstrap credential without returning the raw credential to a human, HTTP client, log, or agent.

```text
Render startup
    |
    v
workspace from HOOKTRY_BOOTSTRAP_WORKSPACE
    |
    v
hooktry://secrets/default-api-token
    |
    | SecretRef + "Bearer " prefix
    v
HttpExecutionProvider
    |
    v
POST /_hooktry/hosted/exposures
    |
    +--> Exposure metadata
    |
    +--> runtime_capability
             |
             v
      AES-256-GCM SecretStore
             |
             v
hooktry://secrets/dogfood-runtime-capability
```

## Enablement

The proof is opt-in. Set:

- `HOOKTRY_BOOTSTRAP_WORKSPACE` to the workspace slug
- `HOOKTRY_DOGFOOD_EXPOSURE_PORT` to a target port, or `self` to target the hosted service's own bound port
- optionally `HOOKTRY_DOGFOOD_EXPOSURE_NAME`; default is `hooktry-dogfood`

If dogfood is enabled without a bootstrap workspace, startup fails closed.

## Safety properties

The API token is resolved only inside `HttpExecutionProvider`. The provider constructs the Authorization header from the SecretRef and a non-secret `Bearer ` prefix.

The Exposure response's `runtime_capability` is captured immediately into the encrypted Workspace SecretStore as `hooktry://secrets/dogfood-runtime-capability`. The value is replaced by `[REDACTED]` before execution evidence is inspected.

Logs contain only the Exposure ID, public URL, target port, and whether the record was newly created or already present.

## Idempotence and readiness

Before provisioning, HOOKTRY checks the Workspace's durable Exposure records. A matching Exposure is reused only when the encrypted `dogfood-runtime-capability` still authorizes that exact Exposure. Stale same-name records and capabilities are revoked before reprovisioning.

A new deployment can begin before its public URL is ready to accept the self-request. Provisioning therefore performs a bounded readiness retry: at most 20 attempts with 500 ms between attempts.

## DOGFOOD2 data-plane proof

With `HOOKTRY_DOGFOOD_EXPOSURE_PORT=self`, HOOKTRY:

1. resolves the captured runtime capability only inside the hosted process
2. attaches a WebSocket runtime to the same process over a loopback runtime URL
3. points the runtime target at the hosted process's own bound HTTP port
4. requests `GET <public exposure>/healthz`
5. accepts the proof only when the relayed response is the hosted relay health response

The public verification is bounded to 60 attempts with one second between attempts so it can tolerate Render's rolling cutover without becoming a permanent monitor.

The resulting proof is end-to-end:

```text
bootstrap
  -> encrypted API SecretRef
  -> authenticated hosted execution
  -> durable Exposure
  -> encrypted runtime-capability SecretRef
  -> WebSocket runtime registration
  -> public ingress
  -> relay broker
  -> runtime proxy
  -> 127.0.0.1:<hosted-port>/healthz
  -> relayed 200 response
```

Neither the API token nor runtime capability is included in startup logs, execution evidence, or readiness logs.


## DOGFOOD3 ask-approve-act-prove proof

After the public relay data-plane proof succeeds, the hosted process proves CONTROL1 through its own public control boundary using only the encrypted `hooktry://secrets/default-api-token`.

The proof uses a harmless exact action:

```text
GET <HOOKTRY_PUBLIC_BASE_URL>/healthz
```

The startup acceptance performs:

1. ask - POST the exact health request to `/_hooktry/hosted/approvals`
2. verify the returned pending ApprovalRecord exposes only the redacted action summary
3. approve - POST `{"decision":"approve"}` through the separate `requests:approve` scope
4. mutation proof - attempt to execute the approval with `/llms.txt` instead and require `approval_request_mismatch`
5. act - resubmit the exact approved `/healthz` request
6. prove - require a consumed ApprovalRecord and successful EXEC4 ExecutionRecord
7. outbox proof - require the matching durable `approval_requested` intent to exist for the new approval
8. inbox ask proof - require the new pending approval to appear in `GET /_hooktry/hosted/approvals`
9. approve - decide the exact approval
10. inbox decision proof - require that approval to disappear from the pending inbox
11. correlate - require `approval.execution_id == execution.execution_id`
12. durable query - fetch `/_hooktry/hosted/executions/{execution_id}` and require its terminal projection to equal the immediate execution proof
13. replay proof - reuse the consumed approval and require `approval_consumed`

Before the control-plane proof begins, HOOKTRY waits until public `/healthz.revision` matches the current `RENDER_GIT_COMMIT`. The ask step still has a bounded readiness retry. Together these prevent a Render rolling cutover from accidentally proving an older revision.

The proof never resolves the API token into logs, generated evidence, or agent context. Every outer control request materializes the operator credential from its destination-bound SecretRef only inside `HttpExecutionProvider`.

Successful completion emits only:

```json
{
  "event": "dogfood_control_plane_ready",
  "approval_id": "<uuid>",
  "execution_id": "<uuid>"
}
```

A failed proof emits `dogfood_control_plane_failed` with a non-secret diagnostic.
