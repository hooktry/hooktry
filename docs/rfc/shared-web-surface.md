# WEB1 - Shared Web Surface

Status: executable vertical slice  
Checked: 2026-10-01

## Decision

ORTYO has one browser application.

It is a React/Vite SPA that depends on canonical ORTYO API and WebSocket contracts, not on Cloudflare APIs, Tauri commands, or Rust internals.

~~~text
                    apps/web
                       |
                canonical ORTYO API
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

1. open ORTYO without an account
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

WEB1 must not expose CLAIM_INTERNAL_TOKEN to browser code.

The current Cloudflare claim endpoint accepts authority from the future authenticated control plane. Until AUTH1 exists, the owner UI may indicate that claim requires sign-in but must not fake claim by embedding internal credentials.

AUTH1 will later exchange authenticated workspace authority plus the owner's claim capability.

## Deployment neutrality

The SPA defaults to same-origin canonical paths.

No Cloudflare SDK is imported by apps/web.

For development, Vite proxies canonical HTTP/WebSocket paths to a local runtime.

For Cloudflare, Workers Static Assets serves the compiled bundle.

For LOCAL1, the Rust binary will serve the same compiled assets.

For DESKTOP1, Tauri may wrap the same bundle. Native OS integration must stay a shell concern and must not fork ORTYO domain semantics.

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
- local asset embedding
- a component library
- Cloudflare-specific browser APIs

Those are later slices over the same web surface.
