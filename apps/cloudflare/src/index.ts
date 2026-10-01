import {
  ANONYMOUS_MAX_BODY_BYTES,
  ANONYMOUS_TTL_SECONDS,
  AdapterError,
  anonymousPrincipal,
  errorResponse,
  json,
  randomCapability,
  randomPrincipal,
  sha256Hex,
  summaryFromRow,
  uuidV7,
  validWorkspaceId,
} from "./core";
import { ExposureRuntime, INTERNAL_EXPOSURE_HEADER } from "./exposure-runtime";
import {
  claimExposure,
  createExposure,
  deleteExposure,
  expiredExposureIds,
  findExposureByIngressCapability,
  findExposureByViewCapability,
  payloadKeysForExposure,
} from "./repository";
import type { AnonymousProvision, Env } from "./types";

export { ExposureRuntime };

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    try {
      const url = new URL(request.url);

      if (request.method === "GET" && url.pathname === "/healthz") {
        return json({ ok: true, service: "ortyo-cloudflare" });
      }

      if (
        request.method === "POST" &&
        url.pathname === "/api/v1/hooks"
      ) {
        return await createHook(request, env);
      }

      const hook = url.pathname.match(/^\/hook\/(hk_[A-Za-z0-9_-]{32})(?:\/.*)?$/);
      if (hook) {
        return await routeHook(request, env, hook[1]);
      }

      const view = url.pathname.match(/^\/view\/(vw_[A-Za-z0-9_-]{32})$/);
      if (view && request.method === "GET") {
        return await routeView(request, env, view[1]);
      }

      const claim = url.pathname.match(/^\/claim\/(cl_[A-Za-z0-9_-]{32})$/);
      if (claim && request.method === "POST") {
        return await routeClaim(request, env, claim[1]);
      }

      return json({ error: { code: "not_found" } }, 404);
    } catch (error) {
      return errorResponse(error);
    }
  },

  async scheduled(
    _controller: ScheduledController,
    env: Env,
    ctx: ExecutionContext,
  ): Promise<void> {
    ctx.waitUntil(cleanupExpired(env));
  },
} satisfies ExportedHandler<Env>;

async function createHook(request: Request, env: Env): Promise<Response> {
  const principal = anonymousPrincipal(request) ?? randomPrincipal();
  const hook = randomCapability("hk_");
  const view = randomCapability("vw_");
  const claim = randomCapability("cl_");
  const exposureId = uuidV7();
  const now = Math.floor(Date.now() / 1000);
  const expiresAt = now + ANONYMOUS_TTL_SECONDS;

  const [principalDigest, ingressCapabilityDigest, viewCapabilityDigest, claimCapabilityDigest] =
    await Promise.all([
      sha256Hex(principal),
      sha256Hex(hook),
      sha256Hex(view),
      sha256Hex(claim),
    ]);

  const exposure = await createExposure(env, {
    exposureId,
    principalDigest,
    ingressCapabilityDigest,
    viewCapabilityDigest,
    claimCapabilityDigest,
    now,
    expiresAt,
  });

  const url = new URL(request.url);
  const base = url.origin;
  const websocketBase =
    url.protocol === "https:"
      ? `wss://${url.host}`
      : `ws://${url.host}`;

  const provision: AnonymousProvision = {
    ...exposure,
    hook_url: `${base}/hook/${hook}`,
    view_url: `${base}/view/${view}`,
    view_websocket_url: `${websocketBase}/view/${view}`,
    claim_url: `${base}/claim/${claim}`,
    anonymous_principal: principal,
  };

  const response = json(provision, 201);
  response.headers.set(
    "set-cookie",
    `ortyo_anon=${principal}; Max-Age=${ANONYMOUS_TTL_SECONDS}; Path=/; HttpOnly; SameSite=Lax${url.protocol === "https:" ? "; Secure" : ""}`,
  );
  response.headers.set("cache-control", "no-store");
  return response;
}

async function routeHook(
  request: Request,
  env: Env,
  token: string,
): Promise<Response> {
  const contentLength = request.headers.get("content-length");
  if (
    contentLength &&
    Number.isFinite(Number(contentLength)) &&
    Number(contentLength) > ANONYMOUS_MAX_BODY_BYTES
  ) {
    throw new AdapterError(413, "body_too_large");
  }

  const row = await findExposureByIngressCapability(env, await sha256Hex(token));
  if (!row) {
    throw new AdapterError(404, "not_found");
  }
  summaryFromRow(row, Math.floor(Date.now() / 1000));

  const headers = new Headers(request.headers);
  headers.set(INTERNAL_EXPOSURE_HEADER, row.exposure_id);
  const internal = new Request(request, { headers });
  return env.EXPOSURES.getByName(row.exposure_id).fetch(internal);
}

async function routeView(
  request: Request,
  env: Env,
  token: string,
): Promise<Response> {
  const row = await findExposureByViewCapability(env, await sha256Hex(token));
  if (!row) {
    throw new AdapterError(404, "not_found");
  }
  summaryFromRow(row, Math.floor(Date.now() / 1000));

  if (request.headers.get("upgrade")?.toLowerCase() === "websocket") {
    const headers = new Headers(request.headers);
    headers.set(INTERNAL_EXPOSURE_HEADER, row.exposure_id);
    return env.EXPOSURES.getByName(row.exposure_id).fetch(
      new Request(request, { headers }),
    );
  }

  return viewerHtml();
}

async function routeClaim(
  request: Request,
  env: Env,
  token: string,
): Promise<Response> {
  if (!env.CLAIM_INTERNAL_TOKEN) {
    throw new AdapterError(503, "claim_auth_unconfigured");
  }

  const authorization = request.headers.get("authorization");
  if (authorization !== `Bearer ${env.CLAIM_INTERNAL_TOKEN}`) {
    throw new AdapterError(401, "unauthorized");
  }

  const workspaceId = request.headers.get("x-ortyo-workspace-id");
  if (!workspaceId || !validWorkspaceId(workspaceId)) {
    throw new AdapterError(400, "invalid_workspace");
  }

  const claimed = await claimExposure(
    env,
    await sha256Hex(token),
    workspaceId,
    Math.floor(Date.now() / 1000),
  );
  return json(claimed);
}

async function cleanupExpired(env: Env): Promise<void> {
  const now = Math.floor(Date.now() / 1000);
  for (const exposureId of await expiredExposureIds(env, now)) {
    const stub = env.EXPOSURES.getByName(exposureId);
    await stub.fetch("https://ortyo.internal/__expire", { method: "POST" });

    const keys = await payloadKeysForExposure(env, exposureId);
    if (keys.length > 0) {
      await env.PAYLOADS.delete(keys);
    }
    await deleteExposure(env, exposureId);
  }
}

function viewerHtml(): Response {
  const html = `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Ortyo webhook viewer</title>
<style>
body{font:14px ui-monospace,SFMono-Regular,Menlo,monospace;margin:0;padding:24px;background:#0b0b0b;color:#f5f5f5}
main{max-width:1100px;margin:auto}h1{font:600 22px system-ui,sans-serif;margin:0 0 8px}
#status{color:#aaa;margin:0 0 20px}pre{white-space:pre-wrap;word-break:break-word;border:1px solid #2a2a2a;padding:16px;border-radius:8px}
</style>
</head>
<body>
<main>
<h1>Ortyo webhook viewer</h1>
<p id="status">Connecting...</p>
<pre id="events"></pre>
</main>
<script>
const status = document.getElementById("status");
const events = document.getElementById("events");
const scheme = location.protocol === "https:" ? "wss:" : "ws:";
const socket = new WebSocket(scheme + "//" + location.host + location.pathname);
socket.onopen = () => { status.textContent = "Live"; };
socket.onclose = () => { status.textContent = "Disconnected"; };
socket.onerror = () => { status.textContent = "Connection error"; };
socket.onmessage = event => {
  try {
    events.textContent += JSON.stringify(JSON.parse(event.data), null, 2) + "\\n";
  } catch {
    events.textContent += event.data + "\\n";
  }
};
</script>
</body>
</html>`;

  return new Response(html, {
    headers: {
      "content-type": "text/html; charset=utf-8",
      "cache-control": "no-store, max-age=0",
      "referrer-policy": "no-referrer",
      "x-content-type-options": "nosniff",
    },
  });
}
