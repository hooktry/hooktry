import {
  ANONYMOUS_MAX_BODY_BYTES,
  ANONYMOUS_MAX_RETAINED_BYTES,
  ANONYMOUS_REQUEST_LIMIT,
  AdapterError,
  errorResponse,
  interactionFromRow,
  json,
  summaryFromRow,
  uuidV7,
} from "./core";
import {
  getExposureById,
  listInteractions,
} from "./repository";
import type {
  AnonymousInteraction,
  Env,
} from "./types";

const INTERNAL_EXPOSURE_HEADER = "x-ortyo-internal-exposure-id";

export class ExposureRuntime {
  private readonly initializing = new Map<WebSocket, AnonymousInteraction[]>();

  constructor(
    private readonly ctx: DurableObjectState,
    private readonly env: Env,
  ) {}

  async fetch(request: Request): Promise<Response> {
    try {
      const url = new URL(request.url);
      if (url.pathname === "/__expire" && request.method === "POST") {
        for (const socket of this.ctx.getWebSockets()) {
          socket.close(1001, "expired");
        }
        return new Response(null, { status: 204 });
      }

      if (request.headers.get("upgrade")?.toLowerCase() === "websocket") {
        return this.openViewer(request);
      }

      if (url.pathname.startsWith("/hook/")) {
        return this.ctx.blockConcurrencyWhile(() => this.capture(request));
      }

      return json({ error: { code: "not_found" } }, 404);
    } catch (error) {
      return errorResponse(error);
    }
  }

  async webSocketMessage(
    _socket: WebSocket,
    _message: string | ArrayBuffer,
  ): Promise<void> {
    // The viewer is server-push only. Client messages intentionally carry no authority.
  }

  async webSocketClose(
    socket: WebSocket,
    code: number,
    reason: string,
    _wasClean: boolean,
  ): Promise<void> {
    this.initializing.delete(socket);
    socket.close(code, reason);
  }

  async webSocketError(socket: WebSocket): Promise<void> {
    this.initializing.delete(socket);
    socket.close(1011, "websocket_error");
  }

  private async capture(request: Request): Promise<Response> {
    const exposureId = request.headers.get(INTERNAL_EXPOSURE_HEADER);
    if (!exposureId) {
      throw new AdapterError(403, "internal_route_denied");
    }

    const row = await getExposureById(this.env, exposureId);
    if (!row) {
      throw new AdapterError(404, "not_found");
    }

    const nowSeconds = Math.floor(Date.now() / 1000);
    summaryFromRow(row, nowSeconds);

    if (row.request_count >= ANONYMOUS_REQUEST_LIMIT) {
      throw new AdapterError(429, "request_limit");
    }

    const body = new Uint8Array(await request.arrayBuffer());
    if (body.byteLength > ANONYMOUS_MAX_BODY_BYTES) {
      throw new AdapterError(413, "body_too_large");
    }
    if (row.retained_bytes + body.byteLength > ANONYMOUS_MAX_RETAINED_BYTES) {
      throw new AdapterError(429, "byte_limit");
    }

    const interactionId = uuidV7();
    const sequence = row.request_count + 1;
    const receivedAtMs = Date.now();
    const bodyKey = `anonymous/${exposureId}/${interactionId}`;
    const contentType = request.headers.get("content-type");

    if (contentType) {
      await this.env.PAYLOADS.put(bodyKey, body, {
        httpMetadata: { contentType },
      });
    } else {
      await this.env.PAYLOADS.put(bodyKey, body);
    }

    const url = new URL(request.url);
    const hook = url.pathname.match(/^\/hook\/[^/]+(\/.*)?$/);
    const path = hook?.[1] ?? "/";
    const query = url.search.length > 1 ? url.search.slice(1) : null;
    const headers = Array.from(request.headers.entries()).filter(
      ([name]) => !name.startsWith("x-ortyo-internal-"),
    );

    try {
      const results = await this.env.DB.batch([
        this.env.DB.prepare(
          `UPDATE anonymous_exposures
           SET request_count = ?, retained_bytes = ?
           WHERE exposure_id = ?
             AND request_count = ?
             AND retained_bytes = ?
             AND (workspace_id IS NOT NULL OR expires_at > ?)`,
        ).bind(
          sequence,
          row.retained_bytes + body.byteLength,
          exposureId,
          row.request_count,
          row.retained_bytes,
          nowSeconds,
        ),
        this.env.DB.prepare(
          `INSERT INTO anonymous_interactions (
             interaction_id, exposure_id, sequence, received_at_ms, method, path, query,
             headers_json, body_key, body_bytes, content_type
           )
           SELECT ?, exposure_id, ?, ?, ?, ?, ?, ?, ?, ?, ?
           FROM anonymous_exposures
           WHERE exposure_id = ?
             AND request_count = ?
             AND retained_bytes = ?`,
        ).bind(
          interactionId,
          sequence,
          receivedAtMs,
          request.method,
          path,
          query,
          JSON.stringify(headers),
          bodyKey,
          body.byteLength,
          contentType,
          exposureId,
          sequence,
          row.retained_bytes + body.byteLength,
        ),
      ]);

      if (results[0].meta.changes !== 1 || results[1].meta.changes !== 1) {
        throw new AdapterError(500, "storage_race");
      }
    } catch (error) {
      await this.env.PAYLOADS.delete(bodyKey);
      throw error;
    }

    const interaction: AnonymousInteraction = {
      interaction_id: interactionId,
      exposure_id: exposureId,
      sequence,
      received_at_unix_ms: receivedAtMs,
      method: request.method,
      path,
      ...(query ? { query } : {}),
      headers,
      ...decodeForLive(body),
      body_bytes: body.byteLength,
    };
    this.broadcast(interaction);

    return json({
      ok: true,
      interaction_id: interactionId,
      sequence,
    });
  }

  private openViewer(request: Request): Response {
    const exposureId = request.headers.get(INTERNAL_EXPOSURE_HEADER);
    if (!exposureId) {
      throw new AdapterError(403, "internal_route_denied");
    }

    const pair = new WebSocketPair();
    const [client, server] = Object.values(pair);
    this.ctx.acceptWebSocket(server);
    this.initializing.set(server, []);

    this.ctx.waitUntil(
      this.initializeViewer(server, exposureId).catch((error) => {
        console.error(error);
        this.initializing.delete(server);
        server.close(1011, "viewer_initialization_failed");
      }),
    );

    return new Response(null, {
      status: 101,
      webSocket: client,
    });
  }

  private async initializeViewer(
    socket: WebSocket,
    exposureId: string,
  ): Promise<void> {
    const row = await getExposureById(this.env, exposureId);
    if (!row) {
      throw new AdapterError(404, "not_found");
    }

    const summary = summaryFromRow(row, Math.floor(Date.now() / 1000));
    socket.send(JSON.stringify({ type: "ready", exposure: summary }));

    let lastSequence = 0;
    for (const stored of await listInteractions(this.env, exposureId)) {
      const interaction = await interactionFromRow(stored, this.env.PAYLOADS);
      socket.send(JSON.stringify({ type: "interaction", interaction }));
      lastSequence = interaction.sequence;
    }

    while (true) {
      const pending = this.initializing.get(socket);
      if (!pending) {
        return;
      }
      if (pending.length === 0) {
        this.initializing.delete(socket);
        return;
      }

      const batch = pending.splice(0).sort((a, b) => a.sequence - b.sequence);
      for (const interaction of batch) {
        if (interaction.sequence <= lastSequence) {
          continue;
        }
        socket.send(JSON.stringify({ type: "interaction", interaction }));
        lastSequence = interaction.sequence;
      }
    }
  }

  private broadcast(interaction: AnonymousInteraction): void {
    const frame = JSON.stringify({ type: "interaction", interaction });
    for (const socket of this.ctx.getWebSockets()) {
      const pending = this.initializing.get(socket);
      if (pending) {
        pending.push(interaction);
        continue;
      }
      try {
        socket.send(frame);
      } catch {
        socket.close(1011, "send_failed");
      }
    }
  }
}

function decodeForLive(
  body: Uint8Array,
): Pick<AnonymousInteraction, "body_encoding" | "body"> {
  try {
    return {
      body_encoding: "utf8",
      body: new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(body),
    };
  } catch {
    return {
      body_encoding: "hex",
      body: Array.from(body, (byte) => byte.toString(16).padStart(2, "0")).join(""),
    };
  }
}

export { INTERNAL_EXPOSURE_HEADER };
