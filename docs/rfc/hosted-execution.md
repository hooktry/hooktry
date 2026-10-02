# EXEC3 - Safe Hosted HTTP Execution with Destination-Bound SecretRefs

Status: executable vertical slice  
Tracking: #77, #87, #97

HOOKTRY can actively initiate a bounded HTTP Interaction from its hosted boundary and return structured Evidence.

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
      "secret_ref": "hooktry://secrets/default-api-token",
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

The hosted API is `POST /_hooktry/hosted/execute` and requires `requests:execute`.

## Evidence and capture

Captured values are written to the Workspace secret store and replaced by `[REDACTED]` before Evidence leaves the executor. The caller receives only a stable reference such as `hooktry://secrets/issued-token`.

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
      "secret_ref": "hooktry://secrets/default-api-token",
      "prefix": "Bearer "
    }
  }
}
```

SecretRef resolution is always scoped to the authenticated Workspace. An `hooktry://secrets/...` reference from one Workspace cannot read the same-named secret from another Workspace. Invalid reference schemes fail closed.

EXEC3 adds an exact HTTP-origin binding to the typed SecretRef path. The binding is non-secret policy metadata stored alongside the encrypted secret lifecycle. Before materializing a typed SecretRef into an outbound header, the executor canonicalizes the request origin and requires an exact match. A mismatch fails before the request is sent with the machine-readable `secret_destination_denied` error.

HTTP capture automatically binds a newly captured SecretRef to the origin that issued the value. A token captured from `https://api.example.com/token` can therefore be reused for `https://api.example.com/...`, but not for `https://other.example.com/...`.

The operator bootstrap credential is bound to `HOOKTRY_PUBLIC_BASE_URL`. Existing operator secrets are upgraded in place by attaching the origin policy; the token does not need to be rotated or exposed.

The legacy string form in `secret_headers` remains available for backward compatibility:

```json
{
  "secret_headers": {
    "authorization": "legacy-secret-name"
  }
}
```

A genuinely legacy unbound secret may still use that form. Once a secret has an origin binding, the binding is enforced regardless of whether the caller uses typed SecretRef syntax or the legacy name syntax, so changing syntax cannot bypass policy. New agent-facing integrations should use typed SecretRefs. An unbound secret used through typed SecretRef syntax fails closed.

Origin binding participates in the normal SECRET2 lifecycle: rotation preserves the current binding, explicit bound rotation can replace it, metadata exposes the non-secret binding, and deletion removes it so the policy cannot leak into a future secret that reuses the same name.

Raw secret values are resolved only inside the executor and are never added to request Evidence. Durable hosted secrets are encrypted at rest with AES-256-GCM and an external `HOOKTRY_SECRETS_KEY`.

## Provider boundary

The domain is not Render-specific. The first hosted executor runs inside the existing HOOKTRY process on Render. Future providers can execute the same Request/Evidence contract in isolated workers, Cloudflare, BYOC, or other runtimes.

Cloudflare's `@cloudflare/computer` is being evaluated separately as a broader sandbox/runtime substrate. It must not silently widen the HTTP-specific authority of EXEC1. See [Cloudflare Computer research](../research/cloudflare-computer.md).

## Dogfood acceptance

The intended first production proof is:

1. execute HOOKTRY's public one-time bootstrap endpoint
2. capture `/credential/token` as a SecretRef
3. ensure Evidence contains only `[REDACTED]`
4. persist the capture with an allowed origin equal to the issuing HOOKTRY origin
5. use the captured SecretRef as Authorization for a hosted Exposure request to that origin
6. prove the same SecretRef is denied for a different origin before any request is sent
7. return only non-secret Exposure evidence to the agent
