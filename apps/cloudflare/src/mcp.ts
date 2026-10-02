import { randomPrincipal } from "./core";
import { provisionAnonymousHook } from "./anonymous-service";
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
        capabilities: { tools: {} },
        instructions:
          "Hooktry exposes a deliberately small public remote tool surface. Private workspace tools require OAuth and are not part of this slice.",
        serverInfo: {
          name: "hooktry",
          version: "0.1.0",
        },
      };
    case "initialize":
      return {
        protocolVersion: negotiatedHandshakeProtocol(rpc),
        capabilities: { tools: {} },
        serverInfo: {
          name: "hooktry",
          version: "0.1.0",
        },
        instructions:
          "Hooktry exposes a deliberately small public remote tool surface. Private workspace tools require OAuth and are not part of this slice.",
      };
    case "ping":
      return {};
    case "tools/list":
      return { tools: tools() };
    case "tools/call":
      return await callTool(rpc.params, request, env);
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
        "Create a temporary Hooktry webhook URL when the user needs an endpoint to receive test webhook or integration traffic. The endpoint expires automatically. This tool does not expose captured request data, viewer capabilities, or claim capabilities.",
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
      },
    },
  ];
}

async function callTool(
  paramsValue: unknown,
  request: Request,
  env: Env,
): Promise<Record<string, unknown>> {
  const params = asRecord(paramsValue);
  const name = params && typeof params.name === "string" ? params.name : null;
  if (!name) {
    return toolError("tools/call requires a tool name");
  }

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
    expires_at_unix_seconds: provision.expires_at_unix_seconds ?? null,
    request_limit: provision.request_limit,
    max_body_bytes: provision.max_body_bytes,
    max_retained_bytes: provision.max_retained_bytes,
  };

  return {
    content: [
      {
        type: "text",
        text: `Created a temporary Hooktry webhook endpoint: ${provision.hook_url}`,
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
