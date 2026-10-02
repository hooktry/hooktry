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
    const response = await worker.fetch(
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
    expect(body.result.tools[0].securitySchemes).toEqual([{ type: "noauth" }]);
    expect(body.result.tools[0].annotations).toEqual({
      readOnlyHint: false,
      destructiveHint: false,
      openWorldHint: false,
      idempotentHint: false,
    });
  });

  it("creates a usable webhook without leaking viewer or claim capabilities", async () => {
    const response = await fetchWorker(
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
      /^https:\/\/hooktry\.com\/hook\/hk_[A-Za-z0-9_-]{32}$/,
    );

    const serialized = JSON.stringify(result);
    for (const forbidden of [
      "view_url",
      "view_websocket_url",
      "claim_url",
      "anonymous_principal",
      "vw_",
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
