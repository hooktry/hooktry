# ORTYO Web

Shared React/Vite surface for ORTYO.

The browser application is intentionally deployment-provider neutral. It talks only to the canonical ORTYO HTTP/WebSocket contract and defaults to same-origin URLs.

The same compiled bundle is intended to run from:

- ORTYO managed cloud
- the local Rust binary
- a future Tauri shell

Cloudflare is only the first static-assets adapter.

## WEB1 flow

~~~text
open app
  -> create ephemeral Hook
  -> receive distinct hook/view/claim capabilities
  -> open view WebSocket
  -> receive ready + durable backlog + live Interactions
  -> inspect body / headers / metadata
~~~

The browser does not receive or embed the Cloudflare internal claim token.

Until AUTH1 exists, the owner UI shows claim as requiring sign-in. The claim capability remains stored only in the owner browser session and is not derivable from a shared view URL.

## Capability handling

After creating a Hook, the full provision response is kept in sessionStorage, keyed by the view capability.

The browser URL becomes:

~~~text
/view/vw_<capability>
~~~

Refreshing the same tab can therefore recover the owner session.

Opening the same URL in another browser/tab without that session state yields a read-only viewer. It can receive Interaction evidence but cannot derive the hook or claim capability.

Session storage is a convenience, not authority on the server.

## Local development

Run the native/Cloudflare-compatible API on port 8787, then:

~~~sh
npm install
npm run dev
~~~

Vite serves the UI on port 5173 and proxies:

- /api
- /hook
- /view including WebSockets
- /claim
- /healthz

to http://127.0.0.1:8787.

The production build has no Cloudflare dependency:

~~~sh
npm run check
~~~

Output is written to dist/.

## Cloudflare adapter

apps/cloudflare/wrangler.jsonc points Workers Static Assets at ../web/dist.

Cloudflare routes API/capability paths through the Worker first:

~~~text
/api/*
/hook/*
/view/*
/claim/*
/healthz
~~~

Normal application assets are served directly by Workers Static Assets.

A normal GET /view/vw_... resolves to the SPA shell. A WebSocket upgrade on exactly that URL remains an authenticated view-capability request handled by the Worker and Durable Object.
