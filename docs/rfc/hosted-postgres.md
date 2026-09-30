# HOSTED8: External Postgres Hosted State

Status: executable vertical slice  
Tracking: #60

HOSTED8 moves hosted authority off an instance-local filesystem without changing the ORTYO hosted lifecycle.

## Backend selection

Production can provide:

```sh
ORTYO_DATABASE_URL=postgresql://user:password@host/database
```

When `ORTYO_DATABASE_URL` is present, both hosted stores use PostgreSQL:

- hosted Exposure metadata
- runtime capability digests / expiry / revocation

Without it, ORTYO keeps using the SQLite adapter selected by `ORTYO_HOSTED_DB_PATH`.

## Domain invariants

The backend does not change:

- Exposure ids
- public URLs
- runtime URLs
- capability scope
- capability expiry
- remote revocation
- broker rebuild through runtime reconnect

The raw runtime capability is never stored in either backend.

## Runtime behavior

The first PostgreSQL adapter uses the synchronous Rust PostgreSQL client behind the existing store API. ORTYO executes database operations through Tokio `spawn_blocking`, keeping network/database blocking work off async request worker threads.

## Secret handling

`ORTYO_DATABASE_URL` is treated as a credential. Config Debug output redacts it and startup JSON emits only the backend kind:

```json
{
  "service": "hosted_relay",
  "runtime_transport": "websocket",
  "storage": "postgres"
}
```

## Render deployment

On Render, ORTYO should use a PostgreSQL instance in the same workspace and region and connect with its internal URL over the private network. The storage contract remains ordinary PostgreSQL and is not tied to Render-specific APIs.
