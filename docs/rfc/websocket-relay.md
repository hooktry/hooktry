# HOSTED2: WebSocket Runtime Transport

Status: executable vertical slice  
Tracking: #40

HOSTED2 adds a deployment-friendly runtime tunnel over WebSocket.

## Why WebSocket

The raw TCP adapter remains useful for local tests and debugging, but a hosted product should not require a second public TCP port. The runtime tunnel can instead use the same normal HTTPS origin as provisioning and public ingress:

```text
443 / HTTPS
  |
  +-- POST /_ortyo/hosted/exposures
  +-- ANY  /e/:exposure_id/*
  +-- WS   /_ortyo/runtime/:exposure_id
```

With TLS termination in front of ORTYO, the runtime uses `wss://`.

## Authentication

The runtime capability is sent in the WebSocket upgrade request:

```http
Authorization: Bearer ortyo_rt_...
```

The capability is never embedded in `public_url` or `runtime_url`. The server validates it before upgrading the connection.

## Provisioning response

HOSTED1 provisioning now also returns:

```json
{
  "runtime_url": "wss://relay.example/_ortyo/runtime/<exposure-id>"
}
```

For a local HTTP test server the scheme is `ws://`; for an HTTPS deployment it is `wss://`.

## Semantics

After authentication the WebSocket adapter reuses the same `RelayBroker`, `RelayFrame`, request ids, Boundary forwarding, and Interaction evidence as the TCP adapter. Multiple requests can be in flight and responses are correlated by request id.

The adapter is transport, not a second domain model.

## Production boundary

HOSTED2 makes standard HTTPS/WSS deployment possible, but does not itself provision certificates or DNS. A deployment platform, reverse proxy, or edge network terminates TLS and forwards the WebSocket upgrade to ORTYO.
