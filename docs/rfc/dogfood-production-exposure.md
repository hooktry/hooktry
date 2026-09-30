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
- `ORTYO_DOGFOOD_EXPOSURE_PORT` to the target port
- optionally `ORTYO_DOGFOOD_EXPOSURE_NAME`; default is `ortyo-dogfood`

If dogfood is enabled without a bootstrap workspace, startup fails closed.

## Safety properties

The API token is resolved only inside `HttpExecutionProvider`. The provider constructs the Authorization header from the SecretRef and a non-secret `Bearer ` prefix.

The Exposure response's `runtime_capability` is captured immediately into the encrypted Workspace SecretStore as `ortyo://secrets/dogfood-runtime-capability`. The value is replaced by `[REDACTED]` before execution evidence is inspected.

Logs contain only the Exposure ID, public URL, target port, and whether the record was newly created or already present.

## Idempotence and readiness

Before provisioning, ORTYO checks the Workspace's durable Exposure records. An existing non-revoked Exposure with the same name and target port is reused.

A new deployment can begin before its public URL is ready to accept the self-request. DOGFOOD1 therefore performs a bounded readiness retry: at most 20 attempts with 500 ms between attempts. It does not run as an unbounded background monitor.

## Scope of this proof

DOGFOOD1 proves the control-plane loop:

```text
bootstrap -> encrypted SecretRef -> authenticated execution -> durable Exposure
```

It does not by itself prove the relay data plane. The Exposure becomes a working tunnel only when a runtime connects with its captured runtime capability and forwards traffic to the configured target port.
