---
name: get-started
description: Use the Hooktry remote plugin when the user needs a temporary webhook endpoint for integration or webhook testing.
---

# Hooktry remote plugin

Use `create_webhook_endpoint` when the user needs a temporary URL that an external service can call during webhook or integration testing. The tool returns the send URL, a separate private viewer URL, and a short-lived one-time browser handoff URL for the same temporary endpoint.

## Current remote surface

The first remote plugin slice intentionally supports endpoint creation only.

The plugin can give the user a browser viewer URL, but it cannot read captured request contents through MCP. Do not claim that the remote plugin itself can inspect captured requests, replay interactions, run Scenarios, access a Workspace, or subscribe to events until those capabilities are actually present in `tools/list`.

## Endpoint handling

The returned `hook_url` is a bearer-capability ingress endpoint. Give it to the webhook sender or integration being tested.

The returned `view_url` is a separate bearer capability for the user to inspect that endpoint in Hooktry. Present it as the private View link. Do not give it to the webhook sender unless the user explicitly asks to share viewing authority.

The returned `handoff_url` is a short-lived, one-time owner handoff for the user to open in a browser. It transfers the owner-side provision into that browser so the user can later claim the Hook through the normal authenticated browser flow. Never give the handoff URL to the webhook sender or integration under test. Do not describe it as a permanent management URL.

In user-facing responses, label this URL **Open in browser**. Do not label it **Handoff** unless the user is explicitly discussing Hooktry's internal capability model. "Handoff" is an implementation/security term; "Open in browser" is the product-facing action.

When the user asks for a webhook and the tool returns `handoff_url`, the final user-facing response MUST include all three links below. Do not omit `Open in browser`, even when the user asked only for a webhook URL. Keep the response concise and clearly separate:

- Send: `hook_url`
- View: `view_url`
- Open in browser: `handoff_url` (one-time; expires at `handoff_expires_at_unix_seconds`)

Treat these fields as operational metadata:

- `exposure_id`
- `expires_at_unix_seconds`
- `handoff_expires_at_unix_seconds`
- `request_limit`
- `max_body_bytes`
- `max_retained_bytes`

The remote tool deliberately does not return the claim capability, anonymous principal, or WebSocket-view capability. The handoff capability is not the claim capability and cannot be reused after successful exchange. Never infer, reconstruct, or invent hidden capabilities from the Hook URL, View URL, Handoff URL, or Exposure ID.

## Safety

- Do not send credentials or unrelated sensitive data to a temporary endpoint unless the user explicitly intends to test that exact payload.
- Do not describe the endpoint as permanent.
- Do not claim that creating an endpoint also creates or authenticates a Hooktry account.
- Follow the live MCP `tools/list` schema as authoritative when the remote surface evolves.
