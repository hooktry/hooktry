# ACCESS3: Persistent Network Relay

Status: executable vertical slice  
Tracking: #14

ACCESS3 moves the ACCESS2 relay protocol across a real network socket.

## What is proven

```text
RelayBroker
    |
    v
TCP relay adapter
    |
    | persistent connection initiated by runtime
    |
    v
HOOKTRY runtime
    |
    v
HTTP Boundary
    |
    v
localhost target
```

The wire protocol uses newline-delimited JSON frames for the first slice. This is deliberately simple and inspectable. The frame model is independent from the eventual production encoding.

Frames:

- `register` / `registered`
- `request`
- `response`
- `ping` / `pong`
- `error`

Request and response frames carry the ACCESS2 `RelayRequest` and `RelayResponse` values unchanged.

## Multiplexing

A single runtime connection can have multiple requests in flight. Responses are correlated by `request_id`, not by wire order.

This matters for webhook bursts and browser applications where serializing all requests behind one slow request would create head-of-line blocking at the application protocol layer.

## Outbound runtime connection

The runtime always initiates the TCP connection to the relay. A reconnecting runtime primitive retries after disconnects, so a developer machine, CI runner, devbox, or agent sandbox does not require inbound network access.

## Heartbeat

The wire protocol has explicit `ping` / `pong` frames. ACCESS3 defines and handles them. ACCESS6 will add production liveness thresholds, lease expiry, and stale connection cleanup.

## Security boundary

ACCESS3 is intentionally not an internet deployment.

It does not yet provide:

- TLS
- runtime authentication
- workspace authorization
- public HTTPS ingress
- DNS
- production rate/size limits

Those are ACCESS4/ACCESS5 concerns. The purpose of this slice is to prove that the transport can become real without contaminating the domain or bypassing the Boundary.
