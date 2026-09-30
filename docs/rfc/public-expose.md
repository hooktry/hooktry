# HOSTED5: Public Exposure Workflow

Status: executable vertical slice  
Tracking: #51

HOSTED5 turns hosted relay primitives into the user-facing public exposure flow.

## User workflow

Run the local ORTYO daemon:

```sh
ortyo serve
```

Then create a public exposure:

```sh
ORTYO_TOKEN='ortyo_...' ortyo expose 3000 web --public
```

The default hosted control plane is:

```text
https://ortyo.onrender.com
```

Override it with `ORTYO_HOSTED_URL`.

## Process ownership

The CLI is intentionally short-lived. It does not own the WebSocket tunnel.

```text
ortyo expose --public
    |
    +-- provision hosted Exposure
    |      -> exposure_id
    |      -> public_url
    |      -> runtime_url
    |      -> per-exposure runtime capability
    |
    +-- POST descriptor to local ortyo serve
           |
           +-- adopt exact Exposure id + public URL
           +-- connect authenticated WebSocket
           +-- wait for Registered
           +-- own reconnect loop
           +-- return verified status

CLI exits
daemon keeps tunnel alive
```

This keeps the command agent-friendly: stdout can be consumed as one JSON result while the daemon owns long-lived network state.

## Authority separation

The workspace-scoped `ORTYO_TOKEN` is used only by the CLI to provision the hosted Exposure. The master `ORTYO_CONTROL_TOKEN` is reserved for bootstrap/admin operations and is never part of the user workflow.

The daemon receives only the short-lived capability scoped to the provisioned Exposure.

The command output contains neither secret.

## Result

A successful public expose returns:

```json
{
  "exposure_id": "...",
  "name": "web",
  "url": "https://ortyo.onrender.com/e/...",
  "access": "public",
  "mode": "relay",
  "target_port": 3000,
  "verified": true,
  "runtime_state": "connected"
}
```

`verified: true` means the hosted relay accepted and registered the daemon's authenticated runtime connection before the CLI returned.

## Lifecycle

`ortyo exposure-revoke <id>` first self-revokes the per-Exposure runtime capability at the hosted relay, removes the broker registration, then aborts the daemon-owned reconnect task and revokes the local Exposure. The old capability cannot reconnect.

Runtime reconnection uses capped exponential backoff after a disconnect.

## Remaining production work

- durable hosted provisioning state
- daemon restart recovery
- custom domains and stable aliases
