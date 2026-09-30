# EXEC2 - Safe Hosted HTTP Execution with SecretRef Chaining

Status: executable vertical slice  
Tracking: #77, #87

ORTYO can actively initiate a bounded HTTP Interaction from its hosted boundary and return structured Evidence.

```text
Workspace credential
      |
      | requests:execute
      v
Request -> ExecutionProvider -> outbound HTTP
                           |
                           v
                     ExecutionEvidence
                           |
                    capture selected values
                           v
                        SecretRef
```

## Request

```json
{
  "method": "POST",
  "url": "https://api.example.com/token",
  "headers": {"accept": "application/json"},
  "secret_headers": {
    "authorization": {
      "secret_ref": "ortyo://secrets/default-api-token",
      "prefix": "Bearer "
    }
  },
  "body": {"action": "issue"},
  "capture": [
    {
      "json_pointer": "/credential/token",
      "secret_name": "issued-token"
    }
  ],
  "timeout_ms": 10000
}
```

The hosted API is `POST /_ortyo/hosted/execute` and requires `requests:execute`.

## Evidence and capture

Captured values are written to the Workspace secret store and replaced by `[REDACTED]` before Evidence leaves the executor. The caller receives only a stable reference such as `ortyo://secrets/issued-token`.

Sensitive response headers including authorization, cookies, and set-cookie are omitted from Evidence.

## Network policy

EXEC1 is an Internet egress boundary, not an open proxy.

The production path:

- permits only HTTP and HTTPS
- resolves the destination before sending
- rejects localhost
- rejects loopback, private, link-local, multicast, unspecified, broadcast, carrier-grade NAT and equivalent IPv6 ranges
- disables redirects instead of following a second unvalidated destination
- rejects explicit Host and Content-Length overrides
- limits request and response bodies to 1 MiB
- limits timeout to 30 seconds

Loopback execution exists only behind an explicit test helper and is not reachable through the hosted API.

## Secrets

Request headers can reference a Workspace secret either by the legacy secret name form or by a typed SecretRef binding. SecretRef bindings can add a non-secret prefix/suffix, which allows forms such as `Authorization: Bearer <secret>` without materializing the secret outside the executor.

```json
{
  "secret_headers": {
    "authorization": {
      "secret_ref": "ortyo://secrets/default-api-token",
      "prefix": "Bearer "
    }
  }
}
```

SecretRef resolution is always scoped to the authenticated Workspace. An `ortyo://secrets/...` reference from one Workspace cannot read the same-named secret from another Workspace. Invalid reference schemes fail closed.

Raw secret values are resolved only inside the executor and are never added to request Evidence. Durable hosted secrets are encrypted at rest with AES-256-GCM and an external `ORTYO_SECRETS_KEY`.

## Provider boundary

The domain is not Render-specific. The first hosted executor runs inside the existing ORTYO process on Render. Future providers can execute the same Request/Evidence contract in isolated workers, Cloudflare, BYOC, or other runtimes.

Cloudflare's `@cloudflare/computer` is being evaluated separately as a broader sandbox/runtime substrate. It must not silently widen the HTTP-specific authority of EXEC1. See [Cloudflare Computer research](../research/cloudflare-computer.md).

## Dogfood acceptance

The intended first production proof is:

1. execute ORTYO's public one-time bootstrap endpoint
2. capture `/credential/token` as a SecretRef
3. ensure Evidence contains only `[REDACTED]`
4. use the captured SecretRef as Authorization for a hosted Exposure request
5. return only non-secret Exposure evidence to the agent
