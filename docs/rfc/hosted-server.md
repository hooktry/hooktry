# HOSTED3: Single-Port Hosted Relay Server

Status: executable vertical slice  
Tracking: #43

HOSTED3 makes the hosted relay runnable as one deployable HTTP service.

## Process

```sh
HOOKTRY_BIND=0.0.0.0:8080 \
HOOKTRY_PUBLIC_BASE_URL=https://relay.example \
hooktry hosted
```

The process serves all hosted surfaces on one listener:

```text
GET  /healthz
GET  /_hooktry/health
POST /_hooktry/hosted/exposures
ANY  /e/:exposure_id/*
WS   /_hooktry/runtime/:exposure_id
```

No public raw TCP relay listener is required. The old TCP adapter remains available for local/debug integration tests, but the single-port hosted server advertises only `runtime_url`.

## Configuration

- `HOOKTRY_BIND` - explicit socket address, for example `0.0.0.0:8080`
- `PORT` - platform-provided port fallback; becomes `0.0.0.0:$PORT`
- `HOOKTRY_PUBLIC_BASE_URL` - externally reachable HTTPS origin used when provisioning public and runtime URLs

If no public base URL is supplied, local development derives a loopback HTTP URL from the bind address. Production deployments should always set `HOOKTRY_PUBLIC_BASE_URL`.

## Startup contract

Startup writes one machine-readable JSON line containing:

- service
- bind
- public_base_url
- runtime_transport

This keeps deployment logs agent-friendly.

## Health

`/healthz` and `/_hooktry/health` return:

```json
{"ok":true,"service":"hosted_relay"}
```

## Deployment boundary

TLS and DNS remain the responsibility of the deployment platform or edge proxy. Because runtime transport is WebSocket, TLS termination naturally turns the provisioned runtime URL into `wss://` while public ingress remains `https://` on the same origin.
