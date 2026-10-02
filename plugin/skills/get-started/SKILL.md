---
name: get-started
description: Use the Hooktry remote plugin when the user needs a temporary webhook endpoint for integration or webhook testing.
---

# Hooktry remote plugin

Use `create_webhook_endpoint` when the user needs a temporary URL that an external service can call during webhook or integration testing.

## Current remote surface

The first remote plugin slice intentionally supports endpoint creation only.

Do not claim that the remote plugin can inspect captured requests, replay interactions, run Scenarios, access a Workspace, or subscribe to events until those authenticated capabilities are actually present in `tools/list`.

## Endpoint handling

The returned `hook_url` is a bearer-capability ingress endpoint. Give it only to the webhook sender or integration being tested.

Treat these fields as operational metadata:

- `exposure_id`
- `expires_at_unix_seconds`
- `request_limit`
- `max_body_bytes`
- `max_retained_bytes`

The remote tool deliberately does not return Hooktry viewer, claim, anonymous-principal, or WebSocket-view capabilities. Never infer, reconstruct, or invent them from the webhook URL or Exposure ID.

## Safety

- Do not send credentials or unrelated sensitive data to a temporary endpoint unless the user explicitly intends to test that exact payload.
- Do not describe the endpoint as permanent.
- Do not claim that creating an endpoint also creates or authenticates a Hooktry account.
- Follow the live MCP `tools/list` schema as authoritative when the remote surface evolves.
