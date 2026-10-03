import { env } from "cloudflare:workers";
import { describe, expect, it } from "vitest";

import worker from "../src/index";
import type { Env } from "../src/types";

const bindings = env as unknown as Env;

function fetchWorker(request: Request): Promise<Response> {
  return worker.fetch(request, bindings);
}

async function callMcp(method: string, params: Record<string, unknown> = {}) {
  return fetchWorker(
    new Request("https://mcp.hooktry.com/mcp", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method,
        params,
      }),
    }),
  );
}

describe("PLUGIN1 remote MCP", () => {
  it("discovers the curated no-auth tool with an MCP Apps result card", async () => {
    const response = await callMcp("tools/list");

    expect(response.status).toBe(200);
    expect(response.headers.get("access-control-allow-origin")).toBe("*");

    const body = await response.json() as any;
    expect(body.result.tools).toHaveLength(1);

    const tool = body.result.tools[0];
    expect(tool.name).toBe("create_webhook_endpoint");
    expect(tool.outputSchema.properties.view_url).toEqual({
      type: "string",
      format: "uri",
    });
    expect(tool.outputSchema.required).toContain("view_url");
    expect(tool.outputSchema.properties.handoff_url).toEqual({
      type: "string",
      format: "uri",
    });
    expect(tool.outputSchema.required).toContain("handoff_url");
    expect(tool.description).toContain("give me a webhook");
    expect(tool.description).toContain("does not need to mention or tag Hooktry explicitly");
    expect(tool.description).toContain("View captured requests read-only");
    expect(tool.description).toContain("Open in Hooktry as owner");
    expect(tool.description).toContain("MCP Apps UI");
    expect(tool._meta.ui.resourceUri).toBe("ui://hooktry/webhook-card-v1.html");
    expect(tool._meta["openai/outputTemplate"]).toBe(
      "ui://hooktry/webhook-card-v1.html",
    );
    expect(tool.securitySchemes).toEqual([{ type: "noauth" }]);
    expect(tool.annotations).toEqual({
      readOnlyHint: false,
      destructiveHint: false,
      openWorldHint: false,
      idempotentHint: false,
    });
  });

  it("advertises resources during initialization", async () => {
    const response = await callMcp("initialize", {
      protocolVersion: "2025-11-25",
      capabilities: {},
      clientInfo: { name: "test", version: "1.0.0" },
    });

    expect(response.status).toBe(200);
    const body = await response.json() as any;
    expect(body.result.capabilities.tools).toEqual({});
    expect(body.result.capabilities.resources).toEqual({});
  });

  it("lists and reads the responsive webhook result card", async () => {
    const listResponse = await callMcp("resources/list");
    expect(listResponse.status).toBe(200);

    const listBody = await listResponse.json() as any;
    expect(listBody.result.resources).toEqual([
      expect.objectContaining({
        uri: "ui://hooktry/webhook-card-v1.html",
        mimeType: "text/html;profile=mcp-app",
      }),
    ]);

    const readResponse = await callMcp("resources/read", {
      uri: "ui://hooktry/webhook-card-v1.html",
    });
    expect(readResponse.status).toBe(200);

    const readBody = await readResponse.json() as any;
    const resource = readBody.result.contents[0];
    expect(resource.uri).toBe("ui://hooktry/webhook-card-v1.html");
    expect(resource.mimeType).toBe("text/html;profile=mcp-app");
    expect(resource._meta.ui.prefersBorder).toBe(false);
    expect(resource._meta["openai/ui"].availableDisplayModes).toEqual([
      "inline",
    ]);
    expect(resource._meta["openai/widgetDescription"]).toContain(
      "do not repeat the long URLs",
    );
    expect(resource._meta["openai/widgetCSP"].redirect_domains).toEqual([
      "https://hooktry.com",
    ]);

    expect(resource.text).toContain("max-width: 680px");
    expect(resource.text).toContain("overflow-wrap: anywhere");
    expect(resource.text).toContain("word-break: break-all");
    expect(resource.text).toContain("user-select: text");
    expect(resource.text).toContain(
      'document.getElementById("hook-url").textContent = state.hook_url',
    );
    expect(resource.text).toContain("handoff_expires_at_unix_seconds");
    expect(resource.text).toContain("max_retained_bytes");
    expect(resource.text).toContain("ui/notifications/tool-result");
  });

  it("creates a usable webhook with viewer, owner handoff, limits, and no claim capability", async () => {
    const response = await worker.fetch(
      new Request("https://mcp.hooktry.com/mcp", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          jsonrpc: "2.0",
          id: 2,
          method: "tools/call",
          params: {
            name: "create_webhook_endpoint",
            arguments: {},
          },
        }),
      }),
      { ...bindings, HOOKTRY_PUBLIC_ORIGIN: "https://hooktry.com" },
    );

    expect(response.status).toBe(200);
    const body = await response.json() as any;
    const result = body.result;

    expect(result.isError).toBe(false);
    expect(result.structuredContent.hook_url).toMatch(
      /^https:\/\/hooktry\.com\/hook\/hk_[0-9a-f]{32}$/,
    );
    expect(result.structuredContent.view_url).toMatch(
      /^https:\/\/hooktry\.com\/view\/vw_[0-9a-f]{32}$/,
    );
    expect(result.structuredContent.handoff_url).toMatch(
      /^https:\/\/hooktry\.com\/open#ho_[0-9a-f]{32}$/,
    );
    expect(
      result.structuredContent.handoff_expires_at_unix_seconds,
    ).toBeTypeOf("number");
    expect(result.structuredContent.request_limit).toBeTypeOf("number");
    expect(result.structuredContent.max_body_bytes).toBeTypeOf("number");
    expect(result.structuredContent.max_retained_bytes).toBeTypeOf("number");
    expect(result.content[0].text).toContain("attached card");
    expect(result.content[0].text).toContain("all three URLs");
    expect(result.content[0].text).not.toContain(result.structuredContent.hook_url);

    const serialized = JSON.stringify(result);
    for (const forbidden of [
      "view_websocket_url",
      "claim_url",
      "anonymous_principal",
      "cl_",
    ]) {
      expect(serialized).not.toContain(forbidden);
    }

    const ingress = await fetchWorker(
      new Request(result.structuredContent.hook_url, {
        method: "POST",
        body: JSON.stringify({ event: "plugin.test" }),
      }),
    );
    expect(ingress.status).toBe(200);

    const viewer = await fetchWorker(
      new Request(result.structuredContent.view_url, {
        headers: { accept: "text/html" },
      }),
    );
    expect(viewer.status).toBe(200);
    expect(await viewer.text()).toContain('<div id="root"></div>');

    const handoffToken = new URL(
      result.structuredContent.handoff_url,
    ).hash.slice(1);
    const exchange = await fetchWorker(
      new Request("https://hooktry.com/api/v1/handoffs/exchange", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ handoff_token: handoffToken }),
      }),
    );
    expect(exchange.status).toBe(200);
    const owner = await exchange.json() as any;
    expect(owner.hook_url).toBe(result.structuredContent.hook_url);
    expect(owner.view_url).toBe(result.structuredContent.view_url);
    expect(owner.claim_url).toMatch(
      /^https:\/\/hooktry\.com\/claim\/cl_[0-9a-f]{32}$/,
    );

    const replay = await fetchWorker(
      new Request("https://hooktry.com/api/v1/handoffs/exchange", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ handoff_token: handoffToken }),
      }),
    );
    expect(replay.status).toBe(410);
  });

  it("accepts initialization notifications without a response body", async () => {
    const response = await fetchWorker(
      new Request("https://mcp.hooktry.com/mcp", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          jsonrpc: "2.0",
          method: "notifications/initialized",
        }),
      }),
    );

    expect(response.status).toBe(202);
    expect(await response.text()).toBe("");
  });
});
