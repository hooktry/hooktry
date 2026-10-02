# ACCESS2: Reverse Relay Protocol

Status: executable vertical slice  
Tracking: #9

## Purpose

ACCESS2 adds the protocol boundary required for hosted Hooktry URLs without binding the core to a tunnel transport or cloud vendor.

The runtime remains behind NAT or a firewall. It participates in the relay from the inside out. Public ingress never connects directly to the application.

```text
external HTTP caller
        |
        v
Hooktry cloud ingress
        |
        v
RelayBroker
        ^
        | runtime-initiated participation
        |
local Hooktry
        |
        v
HTTP Boundary
        |
        +--> Interaction(origin=proxied)
        |
        v
local target service
```

## Relay envelopes

The relay moves two transport-neutral envelopes:

- `RelayRequest` carries request identity, exposure identity, method, path, query, headers, and raw body bytes.
- `RelayResponse` carries the matching request identity, status, headers, and raw body bytes.

The broker is responsible only for correlation and delivery. It does not own HTTP evidence semantics.

## Runtime authority

The local Hooktry runtime resolves the Exposure and forwards the request through the same HTTP proxy path used by local exposures.

This preserves the product invariant:

> Relay transport cannot bypass the Boundary.

Every successful relay round trip therefore produces normal `Interaction` evidence with:

- `origin = proxied`
- `exposure_id`
- `relay_request_id`
- request and response evidence
- duration

## Provider separation

`ExposureService` now selects a provider from the requested `ExposureMode`.

- `forward` uses the local exposure provider.
- `relay` uses a relay exposure provider.

The current relay provider emits deterministic test URLs under `https://relay.hooktry.test`. Production DNS and TLS are intentionally outside this slice.

## Failure semantics

The broker distinguishes:

- `RuntimeUnavailable` - no runtime has registered for the exposure.
- `RuntimeDisconnected` - a runtime was registered but its receiving side is gone.
- `ResponseDropped` - work was delivered but no response came back through the completion channel.

These semantics should survive whichever network transport is selected later.

## What this does not choose

ACCESS2 deliberately does not choose:

- WebSocket vs HTTP/2 vs QUIC
- Cloudflare vs Render vs Fly vs another hosting provider
- global routing
- production authentication
- workspace authorization
- public DNS naming
- billing

Those are adapters and deployment decisions on top of the relay protocol.
