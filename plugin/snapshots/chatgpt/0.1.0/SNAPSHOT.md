# Hooktry ChatGPT plugin snapshot 0.1.0

This directory is the canonical recovery snapshot for the private Hooktry ChatGPT plugin baseline to recreate from scratch.

## Baseline

- Plugin: `hooktry`
- Version: `0.1.0`
- MCP endpoint: `https://mcp.hooktry.com/mcp`
- Discoverability: `PRIVATE`

This is a clean baseline identity. Do not reuse a deleted plugin ID or release ID when recreating it.

## Architecture decision

Hooktry is intentionally a plugin-only ChatGPT integration. It bundles the remote MCP server and onboarding skill directly in the plugin package. An additional ChatGPT App is not required. An empty Apps section in plugin detail is expected and must not be treated as a missing component or used as a workaround for branding issues.

## Logo requirement

The manifests intentionally use 512x512 PNG files for `logo`, `logoDark`, `composerIcon`, and `composerIconDark`.

The SVG source files are retained as editable source. Do not switch the manifests back to SVG-only.

Branding history: on 2026-10-02, Hooktry plugin 0.1.2 rendered the Hooktry plugin icon after switching to the canonical 512x512 Brand Assets PNGs `icon/hooktry-icon-primary-on-light-512.png` and `icon/hooktry-icon-inverse-on-dark-512.png`. The separate App binding still showed a fallback icon, confirming that Plugin and App branding are distinct layers. The App binding was later removed from the intended architecture.

Known regression (2026-10-03): the recreated 0.1.0 plugin renders ChatGPT's generic plugin icon even though its manifests reference bundled 512x512 PNGs. Treat this as a plugin-logo regression. Do not add a ChatGPT App as a workaround. Recovery should use the exact canonical Brand Assets PNG bytes from the known-working 0.1.2 lineage, not PNGs regenerated from SVG source.

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
