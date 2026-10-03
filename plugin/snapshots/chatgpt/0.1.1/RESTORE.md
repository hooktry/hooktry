# Restoring the Hooktry ChatGPT plugin

1. Copy this directory as the plugin package root.
2. Keep the file paths exactly as they are.
3. Keep `plugin.json` at package root and preserve the `com.openai.interface` fields.
4. Keep `logo`, `logoDark`, `composerIcon`, and `composerIconDark` pointed at the PNG assets.
5. Keep `.codex-plugin/plugin.json` as the compatibility manifest.
6. Keep `mcp.json` pointed at `https://mcp.hooktry.com/mcp`.
7. Keep `.mcp.json` pointed at the same Streamable HTTP endpoint and preserve the empty `headers` object.
8. Keep `skills/get-started/SKILL.md` unchanged unless the live MCP contract has intentionally changed.
9. Publish/recreate the plugin as a fresh private plugin if the old plugin identity was deleted. Do not try to reconstruct the old backend plugin ID from memory.
10. After recreation, verify the UI actually renders the Hooktry mark instead of a generic plugin icon before making further UI changes.
11. Verify a generic request such as "give me a webhook" creates an endpoint and that the response preserves Webhook/Send, View, and Open in Hooktry/owner capabilities.
12. Verify capability URLs remain selectable exact strings and only wrap visually on narrow screens.

This snapshot is the source of truth for the deleted 0.1.1 private plugin release. The release ID and old plugin ID are historical provenance, not identifiers to reuse blindly.
