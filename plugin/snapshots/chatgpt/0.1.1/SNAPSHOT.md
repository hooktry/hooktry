# Hooktry ChatGPT plugin snapshot 0.1.1

This directory is the canonical recovery snapshot for the private Hooktry ChatGPT plugin release that was current immediately before deletion/recreation.

## Source release

- Plugin: `hooktry`
- Version: `0.1.1`
- Plugin ID: `plugins_6ac107133fec8191aa0c7471e7aa8860`
- Release ID: `pluginrel_6ac109786b7881918248359b0f5ab55c`
- Release created: `2026-10-03T13:56:08.419919Z`
- Scope: `USER`
- Discoverability: `PRIVATE`
- MCP endpoint: `https://mcp.hooktry.com/mcp`

## Why PNG assets are part of the snapshot

The private 0.1.1 release uses PNG files for `logo`, `logoDark`, `composerIcon`, and `composerIconDark`.

The SVG source files are retained too. The PNG choice is intentional: the ChatGPT plugin UI previously showed a generic icon when the plugin referenced the SVG assets, while the 0.1.1 release was rebuilt with 512x512 PNG assets.

Do not change the manifest back to SVG-only when recreating this release.

## User-facing contract

The plugin is for temporary webhook endpoint creation through `create_webhook_endpoint`.

The current contract includes:

- generic requests such as "give me a webhook" route to Hooktry without requiring `@Hooktry`;
- `hook_url` is the Send/Webhook capability;
- `view_url` is the separate read-only View capability;
- `handoff_url` is a short-lived, one-time owner link presented as **Open in Hooktry** or **Open as owner**;
- MCP Apps UI is the canonical presentation when supported;
- all three capability URLs remain exact single underlying strings;
- responsive wrapping is visual CSS only;
- no inserted newlines, spaces, zero-width characters, ellipses, or separators inside capability URLs;
- the card exposes copy/open actions and operational limits;
- text-only hosts must still surface all three user-facing capabilities and the returned limits;
- the live MCP `tools/list` schema is authoritative if the remote surface changes.

## Snapshot contents

The directory contains the complete plugin package plus recovery notes: manifests, MCP configuration, onboarding skill, light/dark SVG source, and light/dark 512x512 PNG assets.

## Canonical repository relationship

This is a recovery snapshot, not a replacement for the main `plugin/` package. The main package may continue to advance independently. If a future plugin is deleted, start from this snapshot and only then apply explicitly documented newer changes.
