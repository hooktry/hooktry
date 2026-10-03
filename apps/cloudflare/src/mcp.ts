import { provisionAnonymousHook } from "./anonymous-service";
import { randomPrincipal } from "./core";
import {
  WEBHOOK_CARD_HTML,
  WEBHOOK_CARD_MIME_TYPE,
  WEBHOOK_CARD_URI,
} from "./mcp-widget";
import type { Env } from "./types";

const MODERN_PROTOCOL_VERSION = "2026-07-28";
const LATEST_HANDSHAKE_PROTOCOL_VERSION = "2025-11-25";
const LEGACY_HANDSHAKE_PROTOCOL_VERSION = "2025-06-18";

const CORS_HEADERS = {
  "access-control-allow-origin": "*",
  "access-control-allow-methods": "POST, GET, DELETE, OPTIONS",
  "access-control-allow-headers":
    "authorization, content-type, mcp-protocol-version, mcp-session-id",
  "access-control-expose-headers": "Mcp-Session-Id",
};

const SERVER_CAPABILITIES = {
  tools: {},
  resources: {},
};

export async function handleMcp(request: Request, env: Env): Promise<Response> {
  if (request.method === "OPTIONS") {
    return withCors(new Response(null, { status: 204 }));
  }

  if (request.method !== "POST") {
    return withCors(
      new Response(null, {
        status: 405,
        headers: { allow: "POST, OPTIONS" },
      }),
    );
  }

  let rpc: Record<string, unknown>;
  try {
    rpc = (await request.json()) as Record<string, unknown>;
  } catch {
    return withCors(
      Response.json(
        rpcError(null, -32700, "parse error"),
        { status: 400 },
      ),
    );
  }

  const id = rpc.id ?? null;
  const method = typeof rpc.method === "string" ? rpc.method : null;
  if (!method) {
    return withCors(
      Response.json(
        rpcError(id, -32600, "invalid request"),
        { status: 400 },
      ),
    );
  }

  if (!("id" in rpc)) {
    return withCors(new Response(null, { status: 202 }));
  }

  const result = await dispatch(method, rpc, request, env);
  if ("error" in result) {
    return withCors(Response.json(result));
  }

  return withCors(
    Response.json({
      jsonrpc: "2.0",
      id,
      result,
    }),
  );
}

async function dispatch(
  method: string,
  rpc: Record<string, unknown>,
  request: Request,
  env: Env,
): Promise<Record<string, unknown>> {
  switch (method) {
    case "server/discover":
      return {
        resultType: "complete",
        supportedVersions: [MODERN_PROTOCOL_VERSION],
        capabilities: SERVER_CAPABILITIES,
        instructions:
          "Hooktry exposes a deliberately small public remote tool surface. Temporary webhook creation includes an inline result card when the host supports MCP Apps UI. Private workspace tools require OAuth and are not part of this slice.",
        serverInfo: {
          name: "hooktry",
          version: "0.1.1",
        },
      };
    case "initialize":
      return {
        protocolVersion: negotiatedHandshakeProtocol(rpc),
        capabilities: SERVER_CAPABILITIES,
        serverInfo: {
          name: "hooktry",
          version: "0.1.1",
        },
        instructions:
          "Hooktry exposes a deliberately small public remote tool surface. Temporary webhook creation includes an inline result card when the host supports MCP Apps UI. Private workspace tools require OAuth and are not part of this slice.",
      };
    case "ping":
      return {};
    case "tools/list":
      return { tools: tools() };
    case "tools/call":
      return await callTool(rpc.params, request, env);
    case "resources/list":
      return { resources: resources() };
    case "resources/read":
      return readResource(rpc.params);
    default:
      return rpcError(rpc.id ?? null, -32601, "method not found");
  }
}

function negotiatedHandshakeProtocol(
  rpc: Record<string, unknown>,
): string {
  const params = asRecord(rpc.params);
  const requested =
    params && typeof params.protocolVersion === "string"
      ? params.protocolVersion
      : null;

  if (requested === LEGACY_HANDSHAKE_PROTOCOL_VERSION) {
    return LEGACY_HANDSHAKE_PROTOCOL_VERSION;
  }
  if (requested === LATEST_HANDSHAKE_PROTOCOL_VERSION) {
    return LATEST_HANDSHAKE_PROTOCOL_VERSION;
  }
  return LATEST_HANDSHAKE_PROTOCOL_VERSION;
}

function tools(): Array<Record<string, unknown>> {
  const securitySchemes = [{ type: "noauth" }];

  return [
    {
      name: "create_webhook_endpoint",
      title: "Create webhook endpoint",
      description:
        "Create a temporary Hooktry webhook endpoint for integration testing. Use this tool for generic requests such as 'give me a webhook', 'create a webhook URL', or 'give me a temporary endpoint' when Hooktry is available; the user does not need to mention or tag Hooktry explicitly. Returns three distinct capabilities: Send webhooks here (hook_url), View captured requests read-only (view_url), and Open in Hooktry as owner (handoff_url, one-time; use this to continue into owner actions such as claiming/managing the Hook). Hosts that support MCP Apps UI should render the attached Hooktry card instead of repeating the long URLs in prose. Text-only hosts must preserve all three URLs plus the returned limits. The raw claim capability is intentionally not returned.",
      inputSchema: {
        type: "object",
        properties: {},
        required: [],
        additionalProperties: false,
      },
      outputSchema: {
        type: "object",
        properties: {
          exposure_id: {
            type: "string",
            format: "uuid",
          },
          hook_url: {
            type: "string",
            format: "uri",
          },
          view_url: {
            type: "string",
            format: "uri",
          },
          handoff_url: {
            type: "string",
            format: "uri",
          },
          handoff_expires_at_unix_seconds: {
            type: "integer",
          },
          expires_at_unix_seconds: {
            type: ["integer", "null"],
          },
          request_limit: {
            type: "integer",
          },
          max_body_bytes: {
            type: "integer",
          },
          max_retained_bytes: {
            type: "integer",
          },
        },
        required: [
          "exposure_id",
          "hook_url",
          "view_url",
          "handoff_url",
          "handoff_expires_at_unix_seconds",
          "expires_at_unix_seconds",
          "request_limit",
          "max_body_bytes",
          "max_retained_bytes",
        ],
        additionalProperties: false,
      },
      annotations: {
        readOnlyHint: false,
        destructiveHint: false,
        openWorldHint: false,
        idempotentHint: false,
      },
      securitySchemes,
      _meta: {
        securitySchemes,
        ui: {
          resourceUri: WEBHOOK_CARD_URI,
        },
        "openai/outputTemplate": WEBHOOK_CARD_URI,
        "openai/toolInvocation/invoking": "Creating webhook…",
        "openai/toolInvocation/invoked": "Webhook ready",
      },
    },
  ];
}

function resources(): Array<Record<string, unknown>> {
  return [
    {
      uri: WEBHOOK_CARD_URI,
      name: "Hooktry webhook card",
      title: "Hooktry webhook card",
      description:
        "Compact responsive result card for a temporary Hooktry webhook, viewer, owner handoff, and runtime limits.",
      mimeType: WEBHOOK_CARD_MIME_TYPE,
    },
  ];
}

function readResource(paramsValue: unknown): Record<string, unknown> {
  const params = asRecord(paramsValue);
  const uri = params && typeof params.uri === "string" ? params.uri : null;

  if (uri !== WEBHOOK_CARD_URI) {
    return rpcError(null, -32002, "resource not found");
  }

  return {
    contents: [
      {
        uri: WEBHOOK_CARD_URI,
        mimeType: WEBHOOK_CARD_MIME_TYPE,
        text: WEBHOOK_CARD_HTML,
        _meta: {
          ui: {
            prefersBorder: false,
            csp: {
              connectDomains: [],
              resourceDomains: [],
            },
          },
          "openai/ui": {
            availableDisplayModes: ["inline"],
          },
          "openai/widgetDescription":
            "A compact Hooktry card showing the webhook URL, read-only viewer URL, one-time owner link with its remaining lifetime, and current request/body/retention/expiry limits. The card already contains copy/open actions, so do not repeat the long URLs in surrounding prose.",
          "openai/widgetPrefersBorder": false,
          "openai/widgetCSP": {
            connect_domains: [],
            resource_domains: [],
            redirect_domains: ["https://hooktry.com"],
          },
        },
      },
    ],
  };
}

async function callTool(
  paramsValue: unknown,
  request: Request,
  env: Env,
): Promise<Record<string, unknown>> {
  const params = asRecord(paramsValue);
  if (!params || typeof params.name !== "string") {
    return toolError("tools/call requires a tool name");
  }
  const name = params.name;

  if (name !== "create_webhook_endpoint") {
    return toolError(`unknown tool: ${name}`);
  }

  const args = asRecord(params.arguments) ?? {};
  if (Object.keys(args).length > 0) {
    return toolError("create_webhook_endpoint does not accept arguments");
  }

  const provision = await provisionAnonymousHook(
    request,
    env,
    randomPrincipal(),
  );
  const structuredContent = {
    exposure_id: provision.exposure_id,
    hook_url: provision.hook_url,
    view_url: provision.view_url,
    handoff_url: provision.handoff_url,
    handoff_expires_at_unix_seconds:
      provision.handoff_expires_at_unix_seconds,
    expires_at_unix_seconds: provision.expires_at_unix_seconds ?? null,
    request_limit: provision.request_limit,
    max_body_bytes: provision.max_body_bytes,
    max_retained_bytes: provision.max_retained_bytes,
  };

  return {
    content: [
      {
        type: "text",
        text:
          "Created a temporary Hooktry webhook. The attached card contains the send URL, read-only viewer, one-time owner link, and current limits. If the host cannot render the card, present all three URLs and the limits from structuredContent.",
      },
    ],
    structuredContent,
    isError: false,
  };
}

function toolError(message: string): Record<string, unknown> {
  return {
    content: [{ type: "text", text: message }],
    isError: true,
  };
}

function rpcError(
  id: unknown,
  code: number,
  message: string,
): Record<string, unknown> {
  return {
    jsonrpc: "2.0",
    id,
    error: {
      code,
      message,
    },
  };
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

function withCors(response: Response): Response {
  const headers = new Headers(response.headers);
  for (const [name, value] of Object.entries(CORS_HEADERS)) {
    headers.set(name, value);
  }
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}
