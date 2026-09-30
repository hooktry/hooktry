# Anonymous-first Exposure and claim lifecycle

Status: accepted for product design  
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
- a high-entropy public capability in the URL
- a separate high-entropy claim capability, stored server-side only as a hash

A browser cookie or anonymous client id is a convenience principal for rediscovery and rate limiting. It is not proof of ownership and must not authorize claim.

Claim transfers the existing Exposure and its retained history into an authenticated user/workspace without changing the public URL.

## Initial anonymous policy

Start close to the useful part of Webhook.site's free model while preserving stronger abuse controls:

- 7-day absolute TTL
- 100 captured requests per Exposure
- up to 3 active anonymous Exposures per anonymous principal
- one Exposure may be auto-provisioned on first landing-page visit
- additional Exposures require an explicit create action
- request body limit: 1 MiB for anonymous capture
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
POST /api/v1/exposures/anonymous
```

Example response:

```json
{
  "id": "exp_...",
  "url": "https://hook.ortyo.com/e/...",
  "viewer_url": "https://ortyo.com/e/...",
  "claim_url": "https://ortyo.com/claim/...",
  "expires_at": "...",
  "request_limit": 100
}
```

The same operation should be discoverable from `llms.txt`, MCP, CLI help, and agent-oriented documentation.

## Cloudflare shape

Anonymous Exposures must be data, not infrastructure objects.

Do not create one Worker, route, Durable Object class, or DNS record per Exposure. Use one wildcard ingress such as:

```text
https://hook.ortyo.com/e/:capability
```

and resolve the capability to an Exposure record.

Recommended responsibilities:

- Workers: ingress, capability validation, quotas, lightweight request normalization
- D1: Exposure metadata, Interaction metadata, indexes, claim state, expiry state
- R2: raw or large request bodies
- Queues: asynchronous persistence/indexing/cleanup work where appropriate
- Durable Objects + WebSocket Hibernation: live viewer fan-out without polling D1

Avoid one-second browser polling. 1,000 open viewers polling once per second would produce 86.4 million poll requests per day before webhook traffic is counted.

## 1,000-user capacity model

Assume 1,000 visitors each receive one anonymous Exposure and every Exposure reaches the 100-request limit.

That is:

- 1,000 Exposure creates
- 100,000 webhook ingestions
- about 101,000 Worker invocations for create + ingress, excluding UI/static traffic
- about 101,000 primary metadata inserts before indexes/outbox/audit amplification
- 100,000 payload objects if every body is stored separately in R2

Illustrative retained payload volume:

| Average body | 100,000 requests | Seven-day GB-month equivalent |
| --- | ---: | ---: |
| 2 KiB | ~0.2 GB | ~0.05 GB-month |
| 10 KiB | ~1.0 GB | ~0.23 GB-month |
| 64 KiB | ~6.4 GB | ~1.49 GB-month |
| 1 MiB maximum | ~100 GB | ~23.3 GB-month |

This scale is small for Workers and R2. D1 is also comfortable on a paid plan if writes are indexed and batched sensibly, but a single D1 database is single-threaded, so burst handling should use Queues rather than assuming unlimited synchronous write throughput.

The first scaling risk is not 1,000 endpoint records. The first risks are abuse, oversized bodies, synchronous write amplification, and live-view polling.

## CPU and memory model

Keep the hot path streaming and metadata-light:

- do not buffer large bodies in memory when they can be streamed to R2
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
7. public ingress capability remains stable unless explicitly rotated
8. cookie/client id alone can never claim

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

## Non-goals

- cloning every Webhook.site feature
- provisioning Cloudflare infrastructure per endpoint
- using cookies as ownership authority
- exposing unrestricted forwarding or transformation to anonymous users
- requiring signup before the first useful request is captured
