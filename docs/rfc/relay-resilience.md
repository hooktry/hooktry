# ACCESS6: Relay Resilience

Status: executable vertical slice  
Tracking: #30

ACCESS6 makes relay resource use and runtime lifecycle bounded.

## Runtime generations

Each broker registration receives a unique registration id. A newer runtime may supersede an older runtime for the same exposure. Disconnect cleanup uses both the exposure id and registration id, so an old connection cannot accidentally remove the newer route.

## Backpressure

The runtime dispatch queue remains bounded at 16 items. Ingress now uses non-blocking enqueue semantics:

- queue has capacity: request is accepted
- queue is full: `RelayError::Overloaded`
- queue is closed: `RelayError::RuntimeDisconnected`

HTTP ingress maps overload to `503 Service Unavailable`. The relay therefore does not accumulate an unbounded set of producers waiting to enqueue.

## Body limits

HTTP ingress has an explicit configurable `max_body_bytes`, defaulting to 1 MiB in this slice. Oversized requests return `413 Payload Too Large` before entering the relay broker.

The current relay still buffers request bodies. Streaming remains a later transport evolution rather than being implied by this limit.

## Reconnect

Runtime reconnect uses capped exponential backoff, starting at the configured retry delay and capping at 30 seconds. A successful connection cycle resets the delay.

## Lifecycle cleanup

When a network relay connection ends normally, its broker registration is removed using generation-safe compare-and-remove semantics.

## Deferred operational hardening

Production telemetry, jitter, graceful process shutdown orchestration, TLS termination, distributed broker state, and provider-specific deployment remain outside this local executable slice.
