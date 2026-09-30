# ACCESS4: HTTP Relay Ingress

Status: executable vertical slice  
Tracking: #17

ACCESS4 puts a real HTTP server in front of the ACCESS3 network relay.

```text
external HTTP client
        |
        v
relay HTTP ingress
        |
        v
RelayBroker
        |
        v
persistent TCP runtime connection
        |
        v
ORTYO Boundary
        |
        v
localhost target
```

The ingress adapter accepts `/e/{exposure_id}` and `/e/{exposure_id}/{path...}`. It preserves method, path, query, request headers, and raw body bytes in the relay envelope. The runtime response is translated back to HTTP status, headers, and raw response bytes.

Hop-by-hop headers are not forwarded across the relay boundary.

## Failure semantics

- no connected runtime: `503 Service Unavailable`
- disconnected runtime: `503 Service Unavailable`
- runtime misses the bounded deadline: `504 Gateway Timeout`
- response channel disappears: `502 Bad Gateway`

The default relay deadline is 30 seconds and can be configured by the embedding ingress service.

## What the integration proof means

The ACCESS4 integration test uses two real listeners:

1. an HTTP ingress listener accepting the external request;
2. a TCP relay listener carrying the request to the runtime.

The runtime then invokes the existing ORTYO Boundary against an ephemeral localhost target. The target response travels all the way back to the original HTTP client and the local runtime stores the Interaction evidence.

## Still intentionally outside this slice

- TLS termination
- public DNS
- hosted deployment
- runtime authentication
- workspace authorization
- exposure ownership
- rate and body-size policy

Those are deployment and ACCESS5 security concerns. No `ortyo.com` production reachability is claimed by this RFC.
