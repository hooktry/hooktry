# Restoring the Hooktry ChatGPT plugin 0.1.0

1. Copy this directory as the plugin package root.
2. Keep the file paths exactly as they are.
3. Keep `plugin.json` at package root and preserve the `com.openai.interface` fields.
4. Keep `logo`, `logoDark`, `composerIcon`, and `composerIconDark` pointed at PNG assets copied byte-for-byte from the canonical Brand Assets 512x512 icons. Do not regenerate those PNGs from SVG during recovery.
5. Keep `.codex-plugin/plugin.json` as the compatibility manifest.
6. Keep `mcp.json` pointed at `https://mcp.hooktry.com/mcp`.
7. Keep `.mcp.json` pointed at the same Streamable HTTP endpoint and preserve the empty `headers` object.
8. Keep `skills/get-started/SKILL.md` as the behavioral source of truth unless the live MCP contract has intentionally changed.
9. Publish/recreate the plugin as a fresh private plugin identity. Do not reconstruct a deleted backend plugin ID or release ID.
10. Verify the UI renders the Hooktry mark instead of a generic plugin icon. A generic icon is a failed recovery even if the manifest and MCP server load correctly.
11. Verify natural requests such as "give me a webhook" and "create a webhook URL" invoke `create_webhook_endpoint` when appropriate.
12. Verify an agent can invoke the tool autonomously when a larger task needs a temporary inbound HTTP receiver.
13. Verify the result preserves Webhook/Send, View, and Open in Hooktry/owner capabilities.
14. Verify capability URLs remain selectable exact strings and only wrap visually on narrow screens.

The 0.1.0 snapshot is the clean recovery baseline. Historical 0.1.1 data may remain in the repository for provenance, but it is not the baseline to recreate.


## Architecture and logo regression note

Hooktry is intentionally plugin-only: Plugin -> bundled MCP -> https://mcp.hooktry.com/mcp. Do not add a separate ChatGPT App merely because the Apps section is empty.

Known-good branding evidence: plugin 0.1.2 on 2026-10-02 used the canonical Brand Assets 512x512 PNGs and the Plugin icon rendered correctly; the then-separate App icon did not. Current recreated 0.1.0 showing a generic Plugin icon is therefore a regression. Use the exact canonical PNG bytes from `hooktry/brand-assets` and verify rendering before considering the recovery complete.
