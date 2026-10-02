# PROFILE1 - Product Runtime Profiles and Retention Boundaries

Status: accepted architecture/product rule  
Checked: 2026-10-02

## Decision

Hooktry has four first-class runtime/product profiles:

~~~text
1. Community Local
2. Anonymous Cloud
3. Authenticated Cloud
4. Self-hosted / BYOC
~~~

They share canonical Hook/Exposure/Interaction semantics, capability separation, and evidence contracts. They differ in placement, persistence, authority, and operational responsibility.

## Profile matrix

| Profile | Install required | Public reachability | Durable payload/history | Ownership |
| --- | --- | --- | --- | --- |
| Community Local | yes | local by default; optional managed relay | local only | local/capability-based |
| Anonymous Cloud | no | managed public Hook | temporary managed retention | anonymous claimable lifecycle |
| Authenticated Cloud | no | managed public Hook | managed, tier/policy-dependent | user/workspace |
| Self-hosted / BYOC | customer/operator controlled | customer/operator controlled | customer/operator controlled | workspace/operator |

These are operating profiles, not separate domain models.

## Community Local

Community Local is the open-source/native Hooktry experience:

~~~text
hooktry binary
  +-- CLI
  +-- MCP
  +-- local API
  +-- embedded WEB1
  +-- SQLite
  +-- native realtime
~~~

Durable Hook/Interaction evidence remains on the user's machine. No Cloudflare/D1/R2 dependency is required.

### Optional managed public relay

A local runtime may use a Hooktry-managed public relay so Stripe, GitHub, another SaaS, or a teammate can reach it:

~~~text
Internet
   -> Hooktry managed ingress / relay
   -> transient routing
   -> outbound connection from local hooktry
   -> local Boundary / target
   -> local durable Interaction evidence
~~~

The managed relay provides reachability. It is not the durable evidence store for Community Local.

### Zero persistent payload/history retention

For the Community relay profile, Hooktry Cloud must not persist webhook payloads or request history as product data.

The relay may hold bytes transiently while routing and may retain the minimum routing, abuse, security, or accounting metadata needed to operate safely. It must not turn that state into hidden Cloud request history.

Required rules:

- do not durably store request bodies
- do not durably store replayable request history
- do not log request bodies
- do not log secret-bearing request headers
- avoid durable full-header capture
- do not expose Cloud-side request history for this profile

Zero retention means zero persistent webhook payload/history retention, not the impossible claim that infrastructure has literally no transient state or operational metadata.

## Anonymous Cloud

Anonymous Cloud is the no-install, create-first experience:

~~~text
open Hooktry
  -> POST /api/v1/hooks
  -> managed ephemeral Hook
~~~

Cloud temporarily stores the anonymous Hook and bounded evidence according to the anonymous lifecycle policy, currently including fixed expiry, request-count limits, per-request size limits, retained-byte limits, and automatic purge.

This differs intentionally from Community Local plus relay:

~~~text
Community Local
  cloud reachability
  local durable history

Anonymous Cloud
  cloud reachability
  temporary cloud history
~~~

## Anonymous is not Free

Anonymous is an ownership/lifecycle state. Free is a pricing/account tier. They must not be modeled as synonyms.

~~~text
anonymous
  -> no authenticated owner yet
  -> temporary claimable lifecycle

logged-in Free
  -> authenticated user/workspace
  -> persistent inventory
  -> separate quotas/retention policy
~~~

A Hook may move from anonymous ephemeral state into an authenticated workspace and then be governed by Free or paid/durable policy. Reusing anonymous quotas immediately after claim can be a temporary implementation bridge, not the final product model.

## Authenticated Cloud

Authenticated Cloud is the managed SaaS profile after login/claim or authenticated creation.

Hooktry Cloud owns operational responsibility for persistent inventory, managed retention, stable managed ingress, workspace/team access, managed realtime, and durability according to tier/policy.

The architecture must not weaken OSS/local correctness or safety merely to create Cloud value. Cloud monetization should center on durability, retention, continuity, collaboration, scale, and managed operations.

## Self-hosted

Self-hosted Hooktry runs on infrastructure operated by the user or organization. The operator owns ingress, persistence, backups, retention, reliability, upgrade cadence, and secrets.

Self-hosted does not mean running Cloudflare locally. It is a deployment profile over portable ports/adapters and should preserve canonical API/lifecycle semantics.

## BYOC / enrolled compute

BYOC is distinct from both self-hosted and fully managed Cloud. Hooktry Cloud may remain the control plane while selected execution/compute runs on customer infrastructure.

Compute placement is a separate concern from Hook ownership, ingress, evidence, and storage. Hooktry must not become a generic VM/devbox platform merely because it can use customer or provider compute.

Use managed/BYOC compute only when it strengthens the primary integration-development lifecycle.

## Shared surfaces

CLI, MCP, Web, and future Tauri are surfaces over the same canonical Hooktry model. They are not separate products.

~~~text
canonical Hooktry model/API
  +-- CLI
  +-- MCP
  +-- Web
       +-- Tauri later
~~~

Agents must be able to use Hooktry without browser state. Humans should be able to inspect the same resources in WEB1 without the UI becoming the source of truth.

## Shared Web invariant

Hooktry has one browser application source:

~~~text
apps/web
  +-- managed Cloud static assets
  +-- embedded in native binary
  +-- self-hosted server
  +-- future Tauri shell
~~~

Provider-specific delivery may differ. Browser semantics must not fork by deployment profile.

## Native binary role

The native Rust binary is a first-class product artifact, not merely a developer helper. It may provide local UI/API, CLI, MCP, local persistence, optional public relay participation, authenticated Cloud access, and future self-host/runtime roles.

Homebrew, curl/bootstrap installers, packages, or signed release artifacts are distribution layers over the same binary.

## Cloudflare boundary

Cloudflare is the first Managed Cloud provider, not the definition of Hooktry Cloud.

The Cloudflare TypeScript adapter may implement canonical behavior with Workers, D1, R2, Durable Objects, and WebSocket Hibernation. The Rust core does not need to be rewritten in TypeScript.

Portability is enforced by contracts and conformance, not by forcing every provider to execute the same implementation language. Namespace, AWS, or another future provider may implement the same managed profile differently.

## Relay plane vs storage plane

Public reachability and retention are separate decisions.

~~~text
Community Local + managed relay
  relay plane: managed
  storage plane: local

Anonymous Cloud
  relay/ingress plane: managed
  storage plane: temporary managed

Authenticated Cloud
  relay/ingress plane: managed
  storage plane: managed

Self-hosted
  relay/ingress plane: operator-selected
  storage plane: operator-selected
~~~

Giving a user a public URL must not silently imply that Hooktry Cloud persists their request history.

## Product boundary

Hooktry is a programmable integration boundary, not infrastructure lifecycle management for its own sake.

Managed compute, VMs, sandboxes, containers, and devboxes are replaceable substrate unless they directly improve:

~~~text
receive / capture
  -> inspect
  -> respond / relay / replay
  -> verify
~~~

Do not introduce generic VM inventory, remote-shell product semantics, checkpoint/lease machinery, or provider-specific compute objects into the Hooktry domain unless a real integration-development job requires them.

## Same-semantics rule

Deployment profiles may differ in adapters, placement, persistence, and operational guarantees. Where a capability exists, they should preserve canonical meaning:

- Hook ingress/view/claim separation
- Interaction/evidence semantics
- claim lifecycle
- browser API shape
- structured agent outputs
- replay/assertion contracts

Provider-specific features may extend a profile but must not silently redefine Hooktry ontology.

## Product progression

~~~text
Community Local
  -> optional zero-retention public relay

Anonymous Cloud
  -> prove value
  -> claim
  -> Authenticated Cloud
       +-- Free policy
       +-- paid durability/retention
       +-- collaboration

Self-host / BYOC
  -> chosen for infrastructure ownership, compliance, or placement
~~~

These are compatible operating modes, not mandatory upgrade steps.

## Relationship to existing decisions

- PORTS1 defines provider-neutral runtime ports and conformance.
- LOCAL1 defines the embedded native Web runtime.
- WEB1 defines the shared React surface.
- Anonymous-first defines temporary managed Hooks and claim.
- DISC1 defines anonymous rediscovery and cross-device handoff.
- ACCESS2/3 define reverse-relay primitives.
- ORCH1 defines GitHub Actions as canonical release/deployment orchestration.
- FUTURE/MONETIZATION defines reliance-oriented Cloud monetization.
- Market scope/vectors define webhook product development/testing/pilot as the primary cohort and managed compute as substrate.

PROFILE1 connects those decisions into one product/runtime mental model.

## Invariants

1. Community Local durable evidence remains local.
2. Community managed relay must not persist webhook payload/history.
3. Anonymous Cloud may persist bounded temporary evidence.
4. Anonymous is not a pricing tier.
5. Authenticated Free is separate policy from anonymous lifecycle.
6. Cloudflare is an adapter, not the product model.
7. CLI/MCP/Web/Tauri share canonical semantics.
8. public reachability does not imply Cloud retention.
9. BYOC compute does not redefine Hook ownership/evidence semantics.
10. self-hosted does not require Cloudflare-compatible infrastructure.
11. managed compute remains replaceable substrate unless the primary product job requires it.
12. deployment profiles may differ operationally but must preserve canonical behavior.

## Non-goals

PROFILE1 does not define final pricing, require Homebrew now, require Tauri, select the final public relay transport, select a second managed-cloud provider, define every self-hosted packaging detail, or turn Hooktry into a VM/devbox platform.
