# HOSTED5: Public Exposure Workflow

Status: executable vertical slice  
Tracking: #51

HOSTED5 turns hosted relay primitives into the user-facing public exposure flow.

## User workflow

Run the local Hooktry daemon:

```sh
hooktry serve
```

Then create a public exposure:

```sh
HOOKTRY_TOKEN='hooktry_...' hooktry expose 3000 web --public
```

The default hosted control plane is:

```text
https://hooktry.onrender.com
```

Override it with `HOOKTRY_HOSTED_URL`.

## Process ownership

The CLI is intentionally short-lived. It does not own the WebSocket tunnel.

```text
hooktry expose --public
    |
    +-- provision hosted Exposure
    |      -> exposure_id
    |      -> public_url
    |      -> runtime_url
    |      -> per-exposure runtime capability
    |
    +-- POST descriptor to local hooktry serve
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

The workspace-scoped `HOOKTRY_TOKEN` is used only by the CLI to provision the hosted Exposure. The master `HOOKTRY_CONTROL_TOKEN` is reserved for bootstrap/admin operations and is never part of the user workflow.

The daemon receives only the short-lived capability scoped to the provisioned Exposure.

The command output contains neither secret.

## Result

A successful public expose returns:

```json
{
  "exposure_id": "...",
  "name": "web",
  "url": "https://hooktry.onrender.com/e/...",
  "access": "public",
  "mode": "relay",
  "target_port": 3000,
  "verified": true,
  "runtime_state": "connected"
}
```

`verified: true` means the hosted relay accepted and registered the daemon's authenticated runtime connection before the CLI returned.

## Lifecycle

`hooktry exposure-revoke <id>` first self-revokes the per-Exposure runtime capability at the hosted relay, removes the broker registration, then aborts the daemon-owned reconnect task and revokes the local Exposure. The old capability cannot reconnect.

Runtime reconnection uses capped exponential backoff after a disconnect.

## Remaining production work

- durable hosted provisioning state
- daemon restart recovery
- custom domains and stable aliases
