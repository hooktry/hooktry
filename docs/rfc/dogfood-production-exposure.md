# DOGFOOD1 - Production Exposure bootstrap

Status: executable vertical slice  
Tracking: #89

DOGFOOD1 proves that ORTYO can use its own durable bootstrap credential without returning the raw credential to a human, HTTP client, log, or agent.

```text
Render startup
    |
    v
workspace from ORTYO_BOOTSTRAP_WORKSPACE
    |
    v
ortyo://secrets/default-api-token
    |
    | SecretRef + "Bearer " prefix
    v
HttpExecutionProvider
    |
    v
POST /_ortyo/hosted/exposures
    |
    +--> Exposure metadata
    |
    +--> runtime_capability
             |
             v
      AES-256-GCM SecretStore
             |
             v
ortyo://secrets/dogfood-runtime-capability
```

## Enablement

The proof is opt-in. Set:

- `ORTYO_BOOTSTRAP_WORKSPACE` to the workspace slug
- `ORTYO_DOGFOOD_EXPOSURE_PORT` to a target port, or `self` to target the hosted service's own bound port
- optionally `ORTYO_DOGFOOD_EXPOSURE_NAME`; default is `ortyo-dogfood`

If dogfood is enabled without a bootstrap workspace, startup fails closed.

## Safety properties

The API token is resolved only inside `HttpExecutionProvider`. The provider constructs the Authorization header from the SecretRef and a non-secret `Bearer ` prefix.

The Exposure response's `runtime_capability` is captured immediately into the encrypted Workspace SecretStore as `ortyo://secrets/dogfood-runtime-capability`. The value is replaced by `[REDACTED]` before execution evidence is inspected.

Logs contain only the Exposure ID, public URL, target port, and whether the record was newly created or already present.

## Idempotence and readiness

Before provisioning, ORTYO checks the Workspace's durable Exposure records. A matching Exposure is reused only when the encrypted `dogfood-runtime-capability` still authorizes that exact Exposure. Stale same-name records and capabilities are revoked before reprovisioning.

A new deployment can begin before its public URL is ready to accept the self-request. Provisioning therefore performs a bounded readiness retry: at most 20 attempts with 500 ms between attempts.

## DOGFOOD2 data-plane proof

With `ORTYO_DOGFOOD_EXPOSURE_PORT=self`, ORTYO:

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
