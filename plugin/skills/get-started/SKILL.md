---
name: get-started
description: Use the Hooktry remote plugin when the user needs a temporary webhook endpoint for integration or webhook testing.
---

# Hooktry remote plugin

Use `create_webhook_endpoint` when the user needs a temporary URL that an external service can call during webhook or integration testing. The tool returns both the send URL and a separate private viewer URL for the same temporary endpoint.

## Current remote surface

The first remote plugin slice intentionally supports endpoint creation only.

The plugin can give the user a browser viewer URL, but it cannot read captured request contents through MCP. Do not claim that the remote plugin itself can inspect captured requests, replay interactions, run Scenarios, access a Workspace, or subscribe to events until those capabilities are actually present in `tools/list`.

## Endpoint handling

The returned `hook_url` is a bearer-capability ingress endpoint. Give it to the webhook sender or integration being tested.

The returned `view_url` is a separate bearer capability for the user to inspect that endpoint in Hooktry. Present it as the private View link. Do not give it to the webhook sender unless the user explicitly asks to share viewing authority.

When the user asks for a webhook, prefer a concise response that clearly separates:

- Send: `hook_url`
- View: `view_url`

Treat these fields as operational metadata:

- `exposure_id`
- `expires_at_unix_seconds`
- `request_limit`
- `max_body_bytes`
- `max_retained_bytes`

The remote tool deliberately does not return the claim capability, anonymous principal, or WebSocket-view capability. Never infer, reconstruct, or invent those values from the Hook URL, View URL, or Exposure ID.

## Safety

- Do not send credentials or unrelated sensitive data to a temporary endpoint unless the user explicitly intends to test that exact payload.
- Do not describe the endpoint as permanent.
- Do not claim that creating an endpoint also creates or authenticates a Hooktry account.
- Follow the live MCP `tools/list` schema as authoritative when the remote surface evolves.
