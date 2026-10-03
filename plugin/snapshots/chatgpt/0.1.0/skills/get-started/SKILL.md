---
name: get-started
description: Use the Hooktry remote plugin when the user or agent needs a temporary webhook endpoint for integration or webhook testing.
---

# Hooktry remote plugin

Use `create_webhook_endpoint` when a user or an agent needs a temporary URL that an external service can call during webhook or integration testing.

## When to use Hooktry

Use Hooktry when the intent is to create a temporary webhook receiver or HTTP endpoint for testing. The request may come directly from a person or indirectly from the model while carrying out another task.

Natural human requests include:

- "give me a webhook"
- "give me a webhook URL"
- "create a webhook"
- "create a webhook URL"
- "give me a temporary endpoint"
- "I need somewhere to receive test webhooks"
- "where can I send a test webhook?"
- "give me an endpoint to receive test requests"

Equivalent wording in other languages should be treated the same way. Do not require the user to mention or tag Hooktry explicitly.

Agent-initiated use is also expected. If another task requires a temporary inbound HTTP receiver, call `create_webhook_endpoint` directly rather than asking the user to repeat the request in Hooktry-specific language.

Do not use Hooktry merely because the conversation mentions webhooks. Use it when a temporary receiver is actually needed. Do not substitute Hooktry for a permanent production endpoint, an existing customer endpoint, or a server the user already specified.

The tool returns the send URL, a separate private viewer URL, and a short-lived one-time browser handoff URL for the same temporary endpoint.

## Current remote surface

The first remote plugin slice intentionally supports endpoint creation only.

The plugin can give the user a browser viewer URL, but it cannot read captured request contents through MCP. Do not claim that the remote plugin itself can inspect captured requests, replay interactions, run Scenarios, access a Workspace, or subscribe to events until those capabilities are actually present in `tools/list`.

## Endpoint handling

The returned `hook_url` is a bearer-capability ingress endpoint. Give it to the webhook sender or integration being tested.

The returned `view_url` is a separate bearer capability for the user to inspect that endpoint in Hooktry. Present it as the private View link. Do not give it to the webhook sender unless the user explicitly asks to share viewing authority.

The returned `handoff_url` is a short-lived, one-time owner handoff for the user to open in a browser. It transfers the owner-side provision into that browser so the user can later claim the Hook through the normal authenticated browser flow. Never give the handoff URL to the webhook sender or integration under test. Do not describe it as a permanent management URL.

In user-facing responses, label this URL **Open in Hooktry** or **Open as owner**. Explain that it is one-time and is the route into owner actions such as claiming/managing the Hook. Do not label it **Handoff** unless the user is explicitly discussing Hooktry's internal capability model. "Handoff" is an implementation/security term.

## MCP Apps UI

When the host supports the Hooktry MCP Apps result card attached to `create_webhook_endpoint`, treat that card as the canonical presentation.

The card already shows:

- the full selectable `hook_url`
- the full selectable `view_url`
- the full selectable `handoff_url`
- copy/open actions
- remaining owner-link lifetime
- request limit
- maximum request body size
- maximum retained bytes
- endpoint expiry

Do not duplicate those long URLs in surrounding prose when the card is rendered. A short acknowledgement is enough.

The card must preserve each URL as one underlying text string. Responsive wrapping is visual CSS only. Do not insert newline characters, spaces, zero-width characters, ellipses, or other separators into capability URLs. Users must be able to select and copy the displayed URL as the exact original value.

On narrow screens the URL may wrap visually. On wider screens it should naturally use fewer lines or a single line. Do not hard-code mobile line breaks.

## Text-only fallback

When the host does not render MCP Apps UI, the final user-facing response MUST include all three user-facing capabilities. Never omit the owner link, even when the user asked only for "a webhook URL".

Prefer this compact fallback:

```markdown
Webhook
`{hook_url}`

[View requests]({view_url}) · [Open in Hooktry]({handoff_url})

{request_limit} requests · {max_body_bytes}/request · {max_retained_bytes} retained
Owner link: {handoff_remaining}; endpoint: {endpoint_remaining}
```

Presentation rules:

- Keep `hook_url` visible as a raw URL because users commonly need to copy it into another service.
- Prefer labeled Markdown links for `view_url` and `handoff_url` instead of printing both long raw URLs. If the client cannot render links, show the raw URLs.
- `View requests` is read-only viewing authority.
- `Open in Hooktry` is the short-lived, one-time owner link used to continue into claim/manage actions.
- Do not call `handoff_url` a claim URL. The raw claim capability is intentionally not returned by this tool.
- Keep operational metadata compact but include the request limit, per-request body limit, retained-byte limit, endpoint expiry, and owner-link lifetime.
- Do not invent or reconstruct missing URLs. The live tool result is authoritative.
- If the live tool result unexpectedly omits `view_url` or `handoff_url`, do not fabricate them. Return the available data and clearly state what the tool omitted.

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
