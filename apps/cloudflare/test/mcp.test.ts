import { env } from "cloudflare:workers";
import { describe, expect, it } from "vitest";

import worker from "../src/index";
import type { Env } from "../src/types";

const bindings = env as unknown as Env;

function fetchWorker(request: Request): Promise<Response> {
  return worker.fetch(request, bindings);
}

describe("PLUGIN1 remote MCP", () => {
  it("discovers the curated no-auth tool", async () => {
    const response = await fetchWorker(
      new Request("https://mcp.hooktry.com/mcp", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          jsonrpc: "2.0",
          id: 1,
          method: "tools/list",
          params: {},
        }),
      }),
    );

    expect(response.status).toBe(200);
    expect(response.headers.get("access-control-allow-origin")).toBe("*");

    const body = await response.json() as any;
    expect(body.result.tools).toHaveLength(1);
    expect(body.result.tools[0].name).toBe("create_webhook_endpoint");
    expect(body.result.tools[0].outputSchema.properties.view_url).toEqual({
      type: "string",
      format: "uri",
    });
    expect(body.result.tools[0].outputSchema.required).toContain("view_url");
    expect(body.result.tools[0].outputSchema.properties.handoff_url).toEqual({
      type: "string",
      format: "uri",
    });
    expect(body.result.tools[0].outputSchema.required).toContain("handoff_url");
    expect(body.result.tools[0].description).toContain("THREE distinct user-facing links");
    expect(body.result.tools[0].description).toContain("View captured requests read-only");
    expect(body.result.tools[0].description).toContain("Open as owner in browser");
    expect(body.result.tools[0].securitySchemes).toEqual([{ type: "noauth" }]);
    expect(body.result.tools[0].annotations).toEqual({
      readOnlyHint: false,
      destructiveHint: false,
      openWorldHint: false,
      idempotentHint: false,
    });
  });

  it("creates a usable webhook with a private viewer and no claim capability", async () => {
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
    expect(result.content[0].text).toContain("SEND REQUESTS HERE:");
    expect(result.content[0].text).toContain("VIEW ONLY (read-only):");
    expect(result.content[0].text).toContain("OPEN TO CLAIM / MANAGE");
    expect(result.content[0].text).toContain("owner link is not the Viewer URL");

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
