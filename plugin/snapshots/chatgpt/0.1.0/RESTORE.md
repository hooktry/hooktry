# Restoring the Hooktry ChatGPT plugin 0.1.0

1. Copy this directory as the plugin package root.
2. Keep the file paths exactly as they are.
3. Keep `plugin.json` at package root and preserve the `com.openai.interface` fields.
4. Keep `logo`, `logoDark`, `composerIcon`, and `composerIconDark` pointed at the PNG assets.
5. Keep `.codex-plugin/plugin.json` as the compatibility manifest.
6. Keep `mcp.json` pointed at `https://mcp.hooktry.com/mcp`.
7. Keep `.mcp.json` pointed at the same Streamable HTTP endpoint and preserve the empty `headers` object.
8. Keep `skills/get-started/SKILL.md` as the behavioral source of truth unless the live MCP contract has intentionally changed.
9. Publish/recreate the plugin as a fresh private plugin identity. Do not reconstruct a deleted backend plugin ID or release ID.
10. Verify the UI renders the Hooktry mark instead of a generic plugin icon.
11. Verify natural requests such as "give me a webhook" and "create a webhook URL" invoke `create_webhook_endpoint` when appropriate.
12. Verify an agent can invoke the tool autonomously when a larger task needs a temporary inbound HTTP receiver.
13. Verify the result preserves Webhook/Send, View, and Open in Hooktry/owner capabilities.
14. Verify capability URLs remain selectable exact strings and only wrap visually on narrow screens.

The 0.1.0 snapshot is the clean recovery baseline. Historical 0.1.1 data may remain in the repository for provenance, but it is not the baseline to recreate.
