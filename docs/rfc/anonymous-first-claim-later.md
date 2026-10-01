# Anonymous-first Exposure and claim lifecycle

Status: executable vertical slice  
Checked: 2026-10-01

## Context

Ortyo should let a human or an agent obtain a usable public HTTP endpoint before authentication or workspace setup.

Product principle:

> Create first. Use/integrate immediately. Claim when valuable.

This is intentionally not a new domain primitive. An anonymous webhook is an `Exposure` with an ephemeral ownership lifecycle.

The entry experience should be similar to Webhook.site: opening the home page may immediately provision a usable endpoint, while agents can provision the same resource through API, MCP, or documented agent instructions.

## Decision

Add an anonymous ephemeral lifecycle to `Exposure`.

A newly provisioned anonymous Exposure has:

- `owner = null`
- `ownership_state = anonymous`
- `expires_at`
- `request_count`
- `request_limit`
- a high-entropy ingress capability in the webhook URL
- a separate high-entropy viewer capability for read access
- a separate high-entropy claim capability, stored server-side only as a hash

A browser cookie or anonymous client id is a convenience principal for rediscovery and rate limiting. It is not proof of ownership and must not authorize claim.

Claim transfers the existing Exposure and its retained history into an authenticated user/workspace without changing the public URL.

## Initial anonymous policy

Start close to the useful part of Webhook.site's free model while preserving stronger abuse controls:

- 5-day absolute TTL
- 100 captured requests per Exposure
- up to 3 active anonymous Exposures per anonymous principal
- one Exposure may be auto-provisioned on first landing-page visit
- additional Exposures require an explicit create action
- request body limit: 5 MiB for anonymous capture
- retained body budget: 50 MiB per Exposure
- expired, unclaimed Exposures and payloads are deleted
- claim before expiry makes the Exposure persistent under normal workspace retention
- no custom domains for anonymous resources
- no arbitrary outbound forwarding for anonymous resources
- no secret-bearing integrations for anonymous resources

The quotas are product policy, not schema invariants. They should be configurable without changing the ontology.

## Human flow

```text
visit ortyo
    |
    +-- Exposure already ready
    |
    +-- send webhook
    |
    +-- inspect Interaction / Recording
    |
    +-- replay / derive assertion where allowed
    |
    +-- claim
            |
            +-- authenticate
            +-- attach to workspace
            +-- preserve URL and retained history
```

The claim prompt should appear after value exists, for example after the first captured request or before an anonymous quota/TTL boundary.

## Agent flow

Agents should not need browser state.

```text
POST /_ortyo/anonymous/exposures
```

Example response:

```json
{
  "exposure": {
    "exposure_id": "...",
    "expires_at_unix_seconds": 0,
    "request_limit": 100,
    "max_body_bytes": 5242880,
    "max_retained_bytes": 52428800
  },
  "hook_url": "https://ortyo.com/hook/hk_Qm8Yp4K2xV7nR3cF1zLt9AbCdEfGhIjK",
  "view_url": "https://ortyo.com/view/vw_N6dT2rX9kP4mJ8sW5qBc7GhJkLmNpQrS",
  "view_websocket_url": "wss://ortyo.com/view/vw_N6dT2rX9kP4mJ8sW5qBc7GhJkLmNpQrS",
  "claim_url": "https://ortyo.com/claim/cl_H3fZ8pR1yK6vM2tQ9xDn4SaBcDeFgHiJ",
  "anonymous_principal": "ortyo_ap_..."
}
```

The same operation should be discoverable from `llms.txt`, MCP, CLI help, and agent-oriented documentation.

## Cloudflare shape

Cloudflare is the first managed-cloud deployment target, not an ORTYO domain dependency. The portable application boundary is defined in [PORTS1](portable-runtime-ports.md); D1, R2, Durable Objects, Queues, and Workers are adapters below that boundary.

Anonymous Exposures must be data, not infrastructure objects.

Do not create one Worker, route, Durable Object class, or DNS record per Exposure. Use one shared origin with capability-specific paths:

```text
https://ortyo.com/hook/hk_<32-char-base64url>
https://ortyo.com/view/vw_<32-char-base64url>
wss://ortyo.com/view/vw_<32-char-base64url>
https://ortyo.com/claim/cl_<32-char-base64url>
```

There are still only three capabilities. The view capability has two transports over the same `vw_` token: HTTPS serves the human browser viewer and WSS serves backlog + live push. Each capability contains 24 cryptographically random bytes (192 bits) encoded as 32 unpadded Base64URL characters. The short prefix identifies the capability kind when the token appears outside its URL. These are bearer capability tokens, not hashes and not database identifiers. The Exposure itself keeps a separate UUIDv7 identity.

Hook, view, and claim capabilities are deliberately different. Giving a webhook sender the hook URL must not grant read or claim authority. Resolve each capability digest to the same Exposure record.

A single Durable Object class with one object instance keyed by active Exposure is acceptable and may be useful for atomic quota/claim state plus live fan-out. That is data sharding, not infrastructure-per-endpoint. Idle instances should be allowed to hibernate.

Recommended responsibilities:

- Workers: ingress, capability validation, quotas, lightweight request normalization
- D1: Exposure metadata, Interaction metadata, indexes, claim state, expiry state
- R2: raw or large request bodies
- Queues: asynchronous persistence/indexing/cleanup work where appropriate
- Durable Objects + WebSocket Hibernation: live viewer fan-out without polling D1

Do not poll. The viewer is push-only over WebSocket. On the current Rust/Axum hosted runtime a broadcast channel fans persisted interactions to connected viewers. A reconnect receives durable backlog before live events. On a Cloudflare deployment, use one Durable Object instance per active Exposure with the WebSocket Hibernation API so idle viewers do not require a resident process or polling requests.

## 1,000-user capacity model

Assume 1,000 visitors each receive one anonymous Exposure and every Exposure reaches the 100-request limit.

That is:

- 1,000 Exposure creates
- 100,000 webhook ingestions
- about 101,000 Worker invocations for create + ingress, excluding UI/static traffic
- about 101,000 primary metadata inserts before indexes/outbox/audit amplification
- 100,000 payload objects if every body is stored separately in R2

Illustrative retained payload volume:

| Average body | 100,000 requests | Five-day GB-month equivalent |
| --- | ---: | ---: |
| 2 KiB | ~0.2 GB | ~0.03 GB-month |
| 10 KiB | ~1.0 GB | ~0.17 GB-month |
| 64 KiB | ~6.4 GB | ~1.07 GB-month |
| 5 MiB per-request ceiling | bounded by 50 MiB/Exposure | bounded by aggregate quota |

This scale is small for Workers and R2. D1 is also comfortable on a paid plan if writes are indexed and batched sensibly, but a single D1 database is single-threaded, so burst handling should use Queues rather than assuming unlimited synchronous write throughput.

The first scaling risk is not 1,000 endpoint records. The first risks are abuse, oversized bodies, synchronous write amplification, and live-view polling.

## CPU and memory model

Keep the hot path streaming and metadata-light:

- current Axum slice accepts a bounded body and persists it in SQLite/Postgres; a Cloudflare adapter should stream raw/large bodies to R2
- do not parse arbitrary payloads deeply on ingress
- cap anonymous body size before expensive work
- hash/normalize only the fields required for evidence and lookup
- perform optional enrichment asynchronously

No memory is reserved per dormant Exposure. A thousand inactive Exposures should cost approximately storage rows, not a thousand resident processes.

## Claim invariants

Claim must be atomic and idempotent.

Required invariants:

1. only the current claim capability can claim
2. claim token is stored hashed
3. token is single-use and rotatable
4. claim cannot resurrect an already-purged Exposure
5. concurrent claims have exactly one winner
6. owner/workspace transition and claim-token invalidation happen atomically
7. public hook capability remains stable unless explicitly rotated
8. cookie/client id alone can never claim
9. hook, view, and claim tokens are distinct capabilities
10. viewer reconnect is backlog + live push, never database polling

## Relationship to existing Ortyo primitives

The anonymous lifecycle should preserve the existing model:

```text
Exposure
  -> Interaction
  -> Recording
  -> Replay
  -> Contract / Assertion
```

Claim changes authority and retention. It does not create a parallel webhook subsystem.

## External validation

Cloudflare's temporary preview accounts use the same broad lifecycle for agent-first deployment: create and use before authentication, return a bearer claim URL, delete unclaimed temporary resources after expiry, and preserve supported resources after claim. See https://developers.cloudflare.com/workers/platform/claim-deployments/.

This validates the product pattern without requiring Ortyo to copy Cloudflare's resource model or 60-minute claim window.

## Non-goals

- cloning every Webhook.site feature
- provisioning Cloudflare infrastructure per endpoint
- using cookies as ownership authority
- exposing unrestricted forwarding or transformation to anonymous users
- requiring signup before the first useful request is captured


## Executable slice

The current hosted server implements:

- `POST /_ortyo/anonymous/exposures` without authentication
- an anonymous-principal cookie/header used only for the three-active-Exposure quota
- `/hook/hk_<token>/*path` for capture
- `GET /view/vw_<token>` as a browser viewer over HTTPS and, with WebSocket upgrade, as backlog + live stream
- `POST /claim/cl_<token>` with workspace `exposures:create` authority
- SQLite and Postgres persistence for Exposure metadata and captured interactions
- SHA-256 digests only for hook/view/claim capabilities at rest
- atomic request-count and retained-byte quota enforcement
- atomic claim and one-shot claim-token invalidation

The existing hook URL remains valid across claim. The current slice intentionally preserves the anonymous request/byte quotas after claim until the authenticated Free tier is specified separately.

## Response policy

Anonymous ingress returns a fixed success response after durable capture. It does not support arbitrary forwarding, arbitrary server-side replay, custom response code, transforms, secrets, or custom domains. Those omissions reduce SSRF/proxy/amplification abuse while preserving the core capture/inspect/claim loop.
