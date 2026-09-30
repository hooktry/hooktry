# HOSTED3: Single-Port Hosted Relay Server

Status: executable vertical slice  
Tracking: #43

HOSTED3 makes the hosted relay runnable as one deployable HTTP service.

## Process

```sh
ORTYO_BIND=0.0.0.0:8080 \
ORTYO_PUBLIC_BASE_URL=https://relay.example \
ortyo hosted
```

The process serves all hosted surfaces on one listener:

```text
GET  /healthz
GET  /_ortyo/health
POST /_ortyo/hosted/exposures
ANY  /e/:exposure_id/*
WS   /_ortyo/runtime/:exposure_id
```

No public raw TCP relay listener is required. The old TCP adapter remains available for local/debug integration tests, but the single-port hosted server advertises only `runtime_url`.

## Configuration

- `ORTYO_BIND` - explicit socket address, for example `0.0.0.0:8080`
- `PORT` - platform-provided port fallback; becomes `0.0.0.0:$PORT`
- `ORTYO_PUBLIC_BASE_URL` - externally reachable HTTPS origin used when provisioning public and runtime URLs

If no public base URL is supplied, local development derives a loopback HTTP URL from the bind address. Production deployments should always set `ORTYO_PUBLIC_BASE_URL`.

## Startup contract

Startup writes one machine-readable JSON line containing:

- service
- bind
- public_base_url
- runtime_transport

This keeps deployment logs agent-friendly.

## Health

`/healthz` and `/_ortyo/health` return:

```json
{"ok":true,"service":"hosted_relay"}
```

## Deployment boundary

TLS and DNS remain the responsibility of the deployment platform or edge proxy. Because runtime transport is WebSocket, TLS termination naturally turns the provisioned runtime URL into `wss://` while public ingress remains `https://` on the same origin.
