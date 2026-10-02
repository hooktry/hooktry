# WEB1 - Shared Web Surface

Status: executable vertical slice  
Checked: 2026-10-01

## Decision

Hooktry has one browser application.

It is a React/Vite SPA that depends on canonical Hooktry API and WebSocket contracts, not on Cloudflare APIs, Tauri commands, or Rust internals.

~~~text
                    apps/web
                       |
                canonical Hooktry API
                       |
        +--------------+--------------+
        |              |              |
        v              v              v
 managed cloud     local binary     Tauri later
 Cloudflare V1     Axum/native      thin shell
~~~

The browser surface is therefore reusable across deployment profiles.

## Initial product slice

WEB1 intentionally implements only the first useful Hook workflow:

1. open Hooktry without an account
2. create an ephemeral Hook
3. copy the public ingress URL
4. connect to the read capability over WebSocket
5. receive ready + durable backlog + live Interaction frames
6. browse Interaction list
7. inspect body, headers, and metadata
8. reconnect automatically if the stream drops

The UI reflects anonymous policy counters and expiry.

## Capability boundary

The create response contains three independent bearer capabilities:

~~~text
hk_...  -> ingress
vw_...  -> view
cl_...  -> claim
~~~

WEB1 preserves that separation.

The owner browser stores the full provision response only in session storage, keyed by the view capability.

A shared/direct /view/vw_... URL has read authority only. If another browser opens that URL without the owner session, the UI does not know and cannot derive:

- hook capability
- claim capability
- anonymous principal

This is a product invariant, not merely UI hiding.

## Claim boundary

WEB1 must not expose `CLAIM_INTERNAL_TOKEN` to browser code.

AUTH1 completes the human claim flow with GitHub sign-in:

~~~text
owner-only claim capability
        +
authenticated Hooktry session
        |
        v
same Exposure becomes persistent in personal workspace
~~~

If the browser has no Hooktry session, the Claim action starts the GitHub OAuth authorization-code + PKCE flow and returns to the same view capability with `?claim=1`. Session storage retains the owner-only provision during the same-tab redirect, so WEB1 can finish the claim without putting the claim capability into an OAuth parameter, cookie, or server-side redirect state.

The internal claim bearer remains CI/control-plane authority only and is never exposed to WEB1.

See [AUTH1 - GitHub sign-in and anonymous Hook claim](github-auth-claim.md).

## Deployment neutrality

The SPA defaults to same-origin canonical paths.

No Cloudflare SDK is imported by apps/web.

For development, Vite proxies canonical HTTP/WebSocket paths to a local runtime.

For Cloudflare, Workers Static Assets serves the compiled bundle.

For LOCAL1, the Rust binary serves the same compiled assets directly from the executable.

For DESKTOP1, Tauri may wrap the same bundle. Native OS integration must stay a shell concern and must not fork Hooktry domain semantics.

## Native local mapping

LOCAL1 embeds the WEB1 dist files into the Rust executable at compile time.

~~~text
hooktry serve / hooktry ui
        |
        +-- canonical Hook API
        +-- native WebSocket viewer
        +-- SQLite
        +-- embedded WEB1 assets
~~~

The local runtime binds to loopback and serves the same SPA paths as the managed cloud profile.

See [LOCAL1 - Embedded Local Web Runtime](local-embedded-web.md).

## Cloudflare mapping

Cloudflare V1 uses Workers Static Assets:

~~~text
/
assets/*
  -> static asset router

/api/*
/hook/*
/view/*
/claim/*
/healthz
  -> Worker first
~~~

A normal view-capability HTTP request is resolved by the Worker for capability validation and then served from the SPA asset binding.

A WebSocket upgrade on the same URL is routed to the Exposure Durable Object.

This preserves one stable view URL for both browser navigation and live stream authority.

## Anonymous rediscovery

WEB1's current owner session is only the first persistence step.

The browser surface should evolve toward a local capability wallet for Recent Hooks so losing a tab does not lose anonymous owner-side capabilities.

A Hook created outside this browser - for example from CLI, MCP, CI, or a remote devbox - must not be inferred from request IP, User-Agent, or cookie state. It requires an explicit one-time cross-device handoff before it can enter the browser wallet.

After authentication, durable cross-device discovery comes from Workspace Inventory.

See [DISC1 - Anonymous Capability Discovery and Cross-device Handoff](capability-discovery-handoff.md).

## State model

Server-side durable history remains authoritative.

The browser keeps only view state and owner-session convenience state.

On stream connection:

~~~text
ready(summary)
backlog interactions
live interactions
~~~

The ready summary already includes durable counters. Backlog frames must therefore not be counted a second time by the UI.

For a new live Interaction with sequence greater than the ready/current request count, the UI may advance its displayed counters until a later server summary refresh.

## UI density

The initial shell is intentionally application-dense rather than landing-page-heavy:

- compact left navigation
- Hook header with public ingress
- usage/expiry metrics
- filterable Interaction list
- body / headers / metadata inspector
- explicit live/reconnecting state

Future Evidence surfaces may occupy the existing navigation without changing the Hook workflow.

## Non-goals

WEB1 does not implement:

- login
- workspace/team management
- real user-facing claim flow
- replay UI
- contract/scenario UI
- Tauri
- a component library
- Cloudflare-specific browser APIs

Those are later slices over the same web surface.
