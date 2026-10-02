# HOSTED6: Remote Runtime Revocation

Status: executable vertical slice  
Tracking: #54

HOSTED6 closes the lifecycle of a hosted runtime capability.

## Self-revocation

The runtime endpoint supports two operations:

```text
GET    /_hooktry/runtime/:exposure_id   WebSocket upgrade
DELETE /_hooktry/runtime/:exposure_id   revoke runtime capability
```

Both use the same per-Exposure capability:

```http
Authorization: Bearer hooktry_rt_...
```

The control-plane token is not required by the daemon and is never copied into daemon state.

## Revoke sequence

When the user runs:

```sh
hooktry exposure-revoke <exposure-id>
```

the local daemon:

1. sends an authenticated DELETE to the hosted runtime endpoint
2. the hosted relay validates and revokes the capability
3. the hosted broker removes the current runtime registration
4. the local daemon aborts its reconnect task
5. the local Exposure becomes revoked

The old capability can no longer register another runtime.

## Failure behavior

Remote revocation is bounded by a short timeout. The daemon always tears down its local tunnel task even if the remote endpoint is unavailable. Capability expiry remains the final safety bound for an unreachable control plane.

## Security invariant

A public URL is routing information, a runtime capability controls exactly one runtime registration, and the workspace/control credential provisions new Exposures. Revoking one runtime capability does not require or expose the broader control credential.
