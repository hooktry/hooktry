# PORTS1 - Portable Runtime Architecture

Status: executable vertical slice  
Checked: 2026-10-01

## Decision

ORTYO is a domain/application system with multiple deployment profiles. Cloudflare, Render, self-hosted infrastructure, and local execution are adapters, not product semantics.

The portability boundary is capability-oriented:

\`\`\`text
Surfaces
  CLI / MCP / Web
        |
Application services
        |
Ports
  repository / realtime / blob / queue / ingress / compute
        |
Adapters
  native / Cloudflare / self-host / future providers
\`\`\`

A deployment provider must not become a domain primitive.

## Portable invariants

Domain and application semantics must not depend on:

- Cloudflare Workers, D1, R2, Queues, or Durable Objects
- Render services or environment metadata
- PostgreSQL or SQLite
- Tokio broadcast channels
- Axum routing
- Redis, NATS, S3, or any future infrastructure product

Provider-specific code may implement a port, compose adapters, expose transport endpoints, and translate provider events into canonical ORTYO commands/events.

## First executable seam

PORTS1 starts with the anonymous Exposure lifecycle because it already needs two different runtime shapes:

- native/self-hosted: SQLite/Postgres + Tokio realtime + Axum WebSocket
- managed Cloudflare target: D1/R2 + Durable Objects/WebSocket Hibernation + Workers

\`AnonymousExposureService\` now depends on two ports:

\`\`\`text
AnonymousExposureRepository
AnonymousInteractionStream
\`\`\`

The native adapters are:

\`\`\`text
AnonymousExposureStore
  -> SQLite or PostgreSQL

TokioAnonymousInteractionStream
  -> tokio::sync::broadcast
\`\`\`

The application service no longer owns a Tokio broadcast map and no longer calls concrete persistence methods directly.

## Native Rust port contract

The Rust traits are a native composition seam. They make local, self-hosted, and native server implementations replaceable without changing application logic.

They are not a requirement that every remote/provider implementation be written in Rust.

A future TypeScript Cloudflare adapter should preserve the same semantic contract through canonical request/event schemas and the anonymous lifecycle conformance suite rather than attempting to implement Rust traits across a language boundary.

## Cloudflare profile

The intended first managed-cloud adapter remains:

\`\`\`text
Worker ingress
    |
Anonymous Exposure application contract
    |
    +-- D1: metadata / indexes / claim state
    +-- R2: raw or large bodies
    +-- Durable Object: per-Exposure realtime authority
    +-- WebSocket Hibernation: live viewers
    +-- Queue: optional async enrichment / cleanup
\`\`\`

Cloudflare-specific APIs must stay below the port boundary.

The Cloudflare adapter may be TypeScript. Portable deterministic Rust logic may later be compiled to WebAssembly selectively when reuse is worth the boundary cost, but ORTYO does not require the full native binary to run inside Workers.

## Deployment profiles

### Local Community

\`\`\`text
ortyo binary
  CLI + MCP + local API + local Web UI
  SQLite
  native realtime
\`\`\`

Optional managed relay provides public reachability with zero persistent cloud payload/history retention. Durable Interaction evidence remains local.

### Managed Cloud

\`\`\`text
ortyo.com
  anonymous -> claim -> authenticated workspace
  managed metadata, payload retention, realtime, teams, policy
\`\`\`

The first implementation may use Cloudflare adapters.

### Self-hosted

\`\`\`text
ORTYO server
PostgreSQL
S3/MinIO or another blob adapter
native/Redis/NATS realtime adapter
operator-owned ingress
\`\`\`

The API and lifecycle semantics remain the same.

### BYOC

ORTYO Cloud may remain the control plane while execution/compute runs on enrolled customer infrastructure. Compute placement is a separate port from webhook ingress/storage.

## Future ports

Do not create one giant \`CloudProvider\` abstraction. Add narrow capability ports when a second real implementation exists or an existing concrete dependency blocks portability.

Likely seams:

- \`PayloadStore\`
- \`Queue\`
- \`IngressProvider\`
- \`ComputeProvider\`
- \`SecretStore\`
- \`IdentityProvider\`
- execution runtime
- hosted Exposure repository/capability repository

## Conformance rule

A provider is an ORTYO implementation only if it preserves canonical behavior.

For anonymous Exposure this includes at minimum:

1. create without authentication
2. distinct hook, view, and claim capabilities
3. request and retained-byte quotas
4. hard expiry for unclaimed resources
5. durable backlog before live push
6. no polling
7. atomic claim
8. claim token single-use
9. stable hook URL across claim
10. viewer capability cannot be used as hook authority
11. hook capability cannot grant view or claim authority

Provider-specific optimizations must not change these semantics.

## Non-goals

PORTS1 does not:

- rewrite the Rust core in TypeScript
- require the Cloudflare adapter to be Rust
- introduce D1/R2/Durable Objects into native code
- build a universal lowest-common-denominator cloud abstraction
- replace working SQLite/Postgres adapters
- force self-hosted users to run Cloudflare-compatible infrastructure
