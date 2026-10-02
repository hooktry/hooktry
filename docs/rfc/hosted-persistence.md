# HOSTED7: Durable Hosted State and Restart Recovery

Status: executable vertical slice  
Tracking: #57

HOSTED7 makes hosted Exposure authority survive process restarts.

## Persisted state

The hosted relay now stores two durable records:

```text
hosted_exposures
    exposure_id
    name
    target_port
    public_url
    runtime_url
    capability_expires_at
    revoked

runtime_capabilities
    SHA-256(token)
    exposure_id
    expires_at
    revoked
```

The raw runtime capability is never persisted.

## Restart invariant

A hosted process may stop and reopen the same state store. An existing daemon can then reconnect with the same unexpired per-Exposure capability and the same Exposure id/public URL.

The broker itself remains ephemeral by design. Active network registrations are rebuilt by runtime reconnect rather than serialized.

```text
process restart
    |
reopen HostedExposureStore
reopen CapabilityStore
    |
broker starts empty
    |
runtime reconnects with existing capability
    |
capability digest authorizes
    |
broker registration rebuilt
    |
same public URL routes again
```

## Revocation invariant

Revocation is also persisted. After a second restart, an old revoked capability is still rejected and the hosted Exposure remains marked revoked.

## SQLite adapter

The first durable adapter is SQLite. Configure it with:

```sh
HOOKTRY_HOSTED_DB_PATH=/var/lib/hooktry/hosted.db
```

The default is `hooktry-hosted.db`.

SQLite is an implementation adapter, not a domain requirement. A future Postgres/D1/other durable adapter can preserve the same hosted lifecycle semantics.

## Deployment caveat

A persistent database file only provides deploy durability if the deployment platform preserves that path across instance replacement. Render Free local filesystem is not a sufficient production durability guarantee by itself.

For production on an ephemeral platform, use a persistent disk or external durable database before relying on hosted Exposures across deploy replacement.
