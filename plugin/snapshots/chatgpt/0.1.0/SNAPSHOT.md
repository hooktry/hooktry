# Hooktry ChatGPT plugin snapshot 0.1.0

This directory is the canonical recovery snapshot for the private Hooktry ChatGPT plugin baseline to recreate from scratch.

## Baseline

- Plugin: `hooktry`
- Version: `0.1.0`
- MCP endpoint: `https://mcp.hooktry.com/mcp`
- Discoverability: `PRIVATE`

This is a clean baseline identity. Do not reuse a deleted plugin ID or release ID when recreating it.

## Logo requirement

The manifests intentionally use 512x512 PNG files for `logo`, `logoDark`, `composerIcon`, and `composerIconDark`.

The SVG source files are retained as editable source. Do not switch the manifests back to SVG-only. The PNG assets were chosen because the ChatGPT plugin UI previously showed a generic icon when the plugin referenced the SVG assets.

## User and agent behavior contract

The plugin is for temporary webhook endpoint creation through `create_webhook_endpoint`.

- Natural English human requests such as "give me a webhook", "give me a webhook URL", "create a webhook", "create a webhook URL", "give me a temporary endpoint", and "I need somewhere to receive test webhooks" should route to Hooktry when the intent is temporary webhook/integration testing.
- Equivalent requests in other languages should be treated the same way.
- The model may invoke Hooktry autonomously when another task requires a temporary inbound HTTP receiver. It should not require the user to say "Hooktry".
- Do not invoke Hooktry merely because webhooks are being discussed.
- Do not use Hooktry as a replacement for a permanent production endpoint or an endpoint the user already supplied.
- `hook_url` is the Send/Webhook capability.
- `view_url` is the separate read-only View capability.
- `handoff_url` is a short-lived, one-time owner link presented as **Open in Hooktry** or **Open as owner**.
- MCP Apps UI is the canonical presentation when supported.
- All three capability URLs remain exact single underlying strings.
- Responsive wrapping is visual CSS only.
- No inserted newlines, spaces, zero-width characters, ellipses, or separators inside capability URLs.
- The card exposes copy/open actions and operational limits.
- Text-only hosts must still surface all three user-facing capabilities and the returned limits.
- The live MCP `tools/list` schema is authoritative if the remote surface changes.

## Snapshot contents

The directory contains the complete plugin package plus recovery notes: manifests, MCP configuration, onboarding skill, light/dark SVG source, and light/dark 512x512 PNG assets.

## Recovery

Start from this directory when recreating the plugin. Preserve file paths and the PNG manifest references. Publish as a fresh private plugin identity. Then verify logo rendering, natural-language routing, agent-initiated creation, capability presentation, and exact URL copy/select behavior before changing anything else.
