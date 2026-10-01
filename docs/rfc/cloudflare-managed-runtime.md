# CF1 - Cloudflare Managed Runtime Adapter

Status: executable vertical slice  
Checked: 2026-10-01

## Decision

Cloudflare is the first managed-cloud implementation of PORTS1. It is not a domain dependency and does not replace the native Rust implementation.

CF1 preserves the anonymous Exposure lifecycle while mapping portable capabilities onto Cloudflare primitives:

```text
HTTP request
    |
Worker
    |
capability digest -> D1
    |
ExposureRuntime Durable Object
    |             |
    |             +-- WebSocket Hibernation -> viewers
    |
    +-- R2 -> raw body
    +-- D1 -> interaction metadata / counters
```

## Resource mapping

- Workers: public routing, capability resolution, body preflight, scheduled cleanup
- D1: Exposure metadata, capability digests, claim state, counters, Interaction metadata
- R2: retained request bodies
- Durable Objects: one logical object per active Exposure for serialized capture and live fan-out
- WebSocket Hibernation: viewers remain connected across object eviction without polling
- Cron Trigger: bounded cleanup of expired unclaimed resources

Queues are intentionally omitted from CF1. There is no asynchronous enrichment/indexing work in the acceptance path yet.

## Public contract

CF1 exposes the canonical anonymous routes:

```text
POST /_ortyo/anonymous/exposures
ANY  /hook/hk_<capability>/*
GET  /view/vw_<capability>
WS   /view/vw_<capability>
POST /claim/cl_<capability>
```

Hook, view, and claim are distinct 192-bit bearer capabilities. Only SHA-256 digests are stored in D1.

The Exposure id and Interaction ids are UUIDv7.

## Capture semantics

The Worker resolves the hook capability to an Exposure id and routes the request to the single Durable Object for that Exposure.

The Durable Object:

1. serializes concurrent capture operations
2. re-checks expiry and quota metadata
3. enforces 5 MiB/request
4. enforces 100 requests/Exposure
5. enforces 50 MiB retained bytes/Exposure
6. writes the body to R2
7. atomically updates D1 counters and inserts Interaction metadata using a D1 batch
8. removes the R2 object if the D1 write fails
9. pushes the canonical Interaction frame to connected viewers

D1/R2 are the durable history. Durable Object memory is not.

## Viewer semantics

The view capability is resolved by the Worker before WebSocket upgrade.

The Durable Object uses the Hibernation WebSocket API. A viewer receives:

```text
ready
durable backlog
live Interaction
live Interaction
...
```

During backlog initialization, live captures are buffered per initializing socket and drained by sequence before the socket joins ordinary live fan-out. This prevents the backlog/live race without polling.

Workers-runtime acceptance tests explicitly evict the Durable Object and prove that the same WebSocket continues to receive subsequent interactions.

## Claim semantics

Claim remains an atomic D1 ownership transition:

- unclaimed and unexpired resource only
- exactly one successful claim
- claim digest is cleared on success
- hook capability and URL remain stable
- retained history remains attached to the same Exposure

CF1 does not implement end-user login/team identity. Workspace authority is supplied by the authenticated control plane through an internal bearer secret plus workspace id. This is an adapter boundary, not a final public auth design.

## Expiry

New requests and new viewer connections reject expired unclaimed Exposures immediately based on D1 state.

A scheduled cleanup pass:

1. finds a bounded batch of expired unclaimed Exposures
2. closes hibernating viewer sockets
3. deletes retained R2 bodies
4. deletes Interaction metadata
5. deletes the Exposure metadata

Claimed Exposures are not deleted by anonymous cleanup.

## Portability

No Rust domain/application module imports Cloudflare APIs.

The Cloudflare implementation is TypeScript because Workers/D1/R2/DO APIs are first-class there. Portability is enforced through public contracts and conformance behavior, not by requiring every deployment profile to execute the same binary.

The native implementation remains:

```text
SQLite/Postgres + Tokio + Axum
```

while CF1 is:

```text
D1 + R2 + Durable Objects + Workers
```

Both must satisfy the anonymous lifecycle invariants defined by PORTS1.

## Non-goals

CF1 does not yet implement:

- React application hosting
- account login, workspaces UI, or team membership
- Free/Paid cloud policy
- Queues-based enrichment
- arbitrary forwarding or transforms
- custom domains
- BYOC compute
- full self-hosted deployment packaging

Those layers must compose with the same canonical API rather than change the anonymous lifecycle.
