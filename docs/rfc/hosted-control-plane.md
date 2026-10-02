# HOSTED1: Control Plane Provisioning

Status: executable vertical slice  
Tracking: #37

HOSTED1 connects the existing relay primitives into a hosted control-plane contract without claiming a production deployment.

## Provision

A runtime asks the hosted control plane for a relay exposure:

```http
POST /_hooktry/hosted/exposures
Content-Type: application/json

{
  "name": "github",
  "target_port": 3000
}
```

The response contains the complete machine contract needed by the runtime:

```json
{
  "exposure_id": "...",
  "name": "github",
  "public_url": "https://relay.example/e/...",
  "relay_addr": "relay.example:7443",
  "runtime_capability": "hooktry_rt_...",
  "capability_expires_at_unix_seconds": 0,
  "target_port": 3000,
  "mode": "relay",
  "access": "public"
}
```

The public URL is routing information. The runtime capability is authority to register that exposure with the relay and is not placed in the public URL.

## Runtime ownership of the target

The cloud control plane does not connect to the developer's local target. The local runtime adopts the provisioned `exposure_id`, binds it to `127.0.0.1:target_port`, and establishes the existing outbound authenticated relay connection.

```text
external caller
    |
public_url
    |
hosted HTTP ingress
    |
RelayBroker
    |
outbound runtime connection + capability
    |
local HOOKTRY Boundary
    |
127.0.0.1:target_port
```

All target traffic still crosses the Boundary and therefore produces canonical Interaction evidence.

## Executable proof

The integration test starts:

1. a real local target HTTP server
2. a real hosted control-plane/HTTP-ingress listener
3. a real relay TCP listener
4. a runtime that adopts the provisioned exposure id
5. an external HTTP caller

It proves request body, query, headers, response, and Interaction evidence survive the full path.

## Deployment boundary

HOSTED1 is provider-neutral. Production deployment still needs encrypted runtime transport or an HTTPS/WebSocket adapter, TLS termination for public ingress, DNS, durable provisioning state, workspace identity, and secret-at-rest hardening.
