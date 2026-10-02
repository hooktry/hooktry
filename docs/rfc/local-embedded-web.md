# LOCAL1 - Embedded Local Web Runtime

Status: executable vertical slice  
Checked: 2026-10-01

## Decision

The HOOKTRY native binary embeds the compiled WEB1 React application and serves it from the same local process as the native API.

~~~text
hooktry
  |
  +-- Rust CLI
  +-- local Axum API
  +-- SQLite
  +-- native anonymous Hook service
  +-- WebSocket live viewer
  +-- embedded apps/web/dist
~~~

There is no separate local frontend implementation.

## Commands

~~~text
hooktry serve
  -> start local API + embedded WEB1 on 127.0.0.1:7777

hooktry ui
  -> same local runtime
  -> best-effort open browser at http://127.0.0.1:7777
~~~

The browser is a convenience only. If automatic opening fails, the printed URL remains usable.

## Canonical local Hook contract

LOCAL1 exposes the same Hook paths used by WEB1:

~~~text
POST /api/v1/hooks
ANY  /hook/hk_<capability>/*
GET  /view/vw_<capability>
WS   /view/vw_<capability>
POST /claim/cl_<capability>
~~~

The local implementation uses the native Rust anonymous service, SQLite persistence, and Tokio realtime adapter.

A normal GET to a valid view capability returns the embedded React shell. A WebSocket upgrade on the same URL enters the native live viewer.

## Storage

Local durable data remains on the user's machine.

The default local database is:

~~~text
hooktry.db
~~~

The native evidence store and anonymous Hook repository use the local SQLite database.

No Cloudflare storage is involved in LOCAL1.

## Single-binary packaging

The shared frontend is compiled first:

~~~text
apps/web
  -> vite build
  -> apps/web/dist
~~~

Cargo build.rs then generates an embedded asset table and rustc includes those exact files into the final executable.

The runtime does not need Node.js, Vite, npm, or external static files.

The repository does not commit generated dist files. Source builds must create the web bundle before compiling Rust.

The supported convenience build is:

~~~sh
bash scripts/build-binary.sh
~~~

which runs WEB1 checks/build and then:

~~~sh
cargo build --release --locked
~~~

The resulting executable is:

~~~text
target/release/hooktry
~~~

## Build boundary

Cargo does not silently download JavaScript dependencies.

If apps/web/dist is missing, build.rs fails with an explicit instruction to build WEB1 first.

CI therefore builds WEB1 before Rust Clippy/tests.

This preserves two properties:

1. the distributed runtime is one native binary
2. ordinary Rust compilation does not hide an npm network side effect

A later release pipeline may produce immutable binaries and Homebrew bottles from the same build order.

## Shared UI invariant

Cloud and local profiles use the same apps/web source and the same browser contract.

~~~text
WEB1
  |
  +-- Cloudflare Static Assets
  |
  +-- native binary embedded assets
  |
  +-- Tauri later
~~~

Provider-specific code may serve the bytes, but it must not fork the UI semantics.

## Security

The local server binds to loopback by default:

~~~text
127.0.0.1:7777
~~~

The embedded SPA applies local response headers including no-sniff, no-referrer, and a restrictive Content Security Policy.

Read, ingress, and claim capabilities remain separate.

LOCAL1 does not turn the browser session into ownership authority.

## Non-goals

LOCAL1 does not yet provide:

- Homebrew packaging
- signed/notarized macOS binaries
- auto-update
- Tauri
- public Internet tunnel for the local machine
- zero-retention managed relay
- user-facing authenticated claim flow

Those remain later slices.
