import { env } from "cloudflare:workers";
import { evictDurableObject } from "cloudflare:test";
import { describe, expect, it } from "vitest";

import {
  ANONYMOUS_MAX_RETAINED_BYTES,
  ANONYMOUS_REQUEST_LIMIT,
} from "../src/core";
import worker from "../src/index";
import { INTERNAL_EXPOSURE_HEADER } from "../src/exposure-runtime";
import type { AnonymousProvision, Env } from "../src/types";

const bindings = env as unknown as Env;
const WORKSPACE_ID = "0199a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5b";

function fetchWorker(request: Request): Promise<Response> {
  return worker.fetch(request, bindings);
}

describe("CF1 ephemeral Hook conformance", () => {
  it("publishes deployment provenance from healthz", async () => {
    const response = await worker.fetch(
      new Request("https://hooktry.test/healthz"),
      { ...bindings, HOOKTRY_RELEASE_SHA: "deadbeef" },
    );

    expect(response.status).toBe(200);
    expect(await response.json()).toEqual({
      ok: true,
      service: "hooktry-cloudflare",
      revision: "deadbeef",
      github_auth_configured: true,
      usage_ingest_configured: true,
    });
  });

  it("publishes Hook, View, and Claim URLs on the canonical public origin", async () => {
    const response = await worker.fetch(
      new Request("https://mcp.hooktry.com/api/v1/hooks", {
        method: "POST",
      }),
      { ...bindings, HOOKTRY_PUBLIC_ORIGIN: "https://hooktry.com" },
    );

    expect(response.status).toBe(201);
    const provision = (await response.json()) as AnonymousProvision;
    expect(provision.hook_url).toMatch(
      /^https:\/\/hooktry\.com\/hook\/hk_[0-9a-f]{32}$/,
    );
    expect(provision.view_url).toMatch(
      /^https:\/\/hooktry\.com\/view\/vw_[0-9a-f]{32}$/,
    );
    expect(provision.view_websocket_url).toMatch(
      /^wss:\/\/hooktry\.com\/view\/vw_[0-9a-f]{32}$/,
    );
    expect(provision.claim_url).toMatch(
      /^https:\/\/hooktry\.com\/claim\/cl_[0-9a-f]{32}$/,
    );
    expect(provision.handoff_url).toMatch(
      /^https:\/\/hooktry\.com\/open#ho_[0-9a-f]{32}$/,
    );
    expect(provision.handoff_expires_at_unix_seconds).toBeTypeOf("number");
    expect(provision.handoff_expires_at_unix_seconds).toBeLessThan(
      provision.expires_at_unix_seconds!,
    );
  });

  it("exchanges a fragment handoff exactly once into owner capabilities", async () => {
    const provision = await createHook();
    const handoff = new URL(provision.handoff_url).hash.slice(1);

    const landing = await fetchWorker(
      new Request("https://hooktry.test/open", {
        headers: { accept: "text/html" },
      }),
    );
    expect(landing.status).toBe(200);
    expect(landing.headers.get("cache-control")).toBe("no-store");
    expect(landing.headers.get("referrer-policy")).toBe("no-referrer");
    expect(landing.headers.get("x-robots-tag")).toBe("noindex, nofollow");

    const exchange = await fetchWorker(
      new Request("https://hooktry.test/api/v1/handoffs/exchange", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ handoff_token: handoff }),
      }),
    );
    expect(exchange.status).toBe(200);
    const owner = (await exchange.json()) as Record<string, unknown>;
    expect(owner.hook_url).toBe(provision.hook_url);
    expect(owner.view_url).toBe(provision.view_url);
    expect(owner.view_websocket_url).toBe(provision.view_websocket_url);
    expect(owner.claim_url).toBe(provision.claim_url);
    expect(owner.anonymous_principal).toBeUndefined();
    expect(owner.handoff_url).toBeUndefined();

    const replay = await fetchWorker(
      new Request("https://hooktry.test/api/v1/handoffs/exchange", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ handoff_token: handoff }),
      }),
    );
    expect(replay.status).toBe(410);
  });

  it(
    "creates, pushes, survives DO eviction, claims, and preserves the hook URL",
    async () => {
    const provision = await withStage("create", createHook());

    expect(provision.hook_url).toMatch(/\/hook\/hk_[0-9a-f]{32}$/);
    expect(provision.view_url).toMatch(/\/view\/vw_[0-9a-f]{32}$/);
    expect(provision.view_websocket_url).toMatch(
      /^wss?:\/\/[^/]+\/view\/vw_[0-9a-f]{32}$/,
    );
    expect(
      provision.view_websocket_url.replace(/^wss?:\/\//, ""),
    ).toBe(provision.view_url.replace(/^https?:\/\//, ""));
    expect(provision.claim_url).toMatch(/\/claim\/cl_[0-9a-f]{32}$/);
    expect(provision.expires_at_unix_seconds).toBeTypeOf("number");

    const viewerResponse = await withStage("viewer-upgrade", fetchWorker(
      new Request(provision.view_url, {
        headers: { Upgrade: "websocket" },
      }),
    ));
    expect(viewerResponse.status).toBe(101);
    const socket = viewerResponse.webSocket;
    if (!socket) {
      throw new Error("expected WebSocket response");
    }
    const inbox = jsonInbox(socket);
    socket.accept();

    const ready = await withStage("viewer-ready", inbox.next());
    expect(ready.type).toBe("ready");
    expect(ready.exposure.exposure_id).toBe(provision.exposure_id);

    const initialSnapshot = await withStage("viewer-snapshot", inbox.next());
    expect(initialSnapshot.type).toBe("snapshot");
    expect(initialSnapshot.interactions).toEqual([]);

    const first = await withStage("first-capture", fetchWorker(
      new Request(`${provision.hook_url}/stripe?delivery=42`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          "stripe-signature": "proof",
        },
        body: JSON.stringify({ type: "checkout.session.completed" }),
      }),
    ));
    expect(first.status).toBe(200);

    const pushed = await withStage("first-push", inbox.next());
    expect(pushed.type).toBe("interaction");
    expect(pushed.interaction.sequence).toBe(1);
    expect(pushed.interaction.path).toBe("/stripe");
    expect(pushed.interaction.query).toBe("delivery=42");
    expect(pushed.interaction.body_encoding).toBe("utf8");

    const claimed = await withStage("claim", fetchWorker(
      new Request(provision.claim_url, {
        method: "POST",
        headers: {
          authorization: "Bearer test-internal-token",
          "x-hooktry-workspace-id": WORKSPACE_ID,
        },
      }),
    ));
    expect(claimed.status).toBe(200);
    const claimedBody = (await claimed.json()) as Record<string, unknown>;
    expect(claimedBody.claimed).toBe(true);
    expect(claimedBody.workspace_id).toBe(WORKSPACE_ID);
    expect(claimedBody.expires_at_unix_seconds).toBeUndefined();

    const handoffAfterClaim = new URL(provision.handoff_url).hash.slice(1);
    const invalidatedHandoff = await withStage(
      "handoff-after-claim",
      fetchWorker(
        new Request("https://hooktry.test/api/v1/handoffs/exchange", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ handoff_token: handoffAfterClaim }),
        }),
      ),
    );
    expect(invalidatedHandoff.status).toBe(410);

    const afterClaim = await withStage("capture-after-claim", fetchWorker(
      new Request(provision.hook_url, {
        method: "POST",
        body: "after-claim",
      }),
    ));
    expect(afterClaim.status).toBe(200);
    const pushedAfterClaim = await withStage("push-after-claim", inbox.next());
    expect(pushedAfterClaim.interaction.sequence).toBe(2);
    expect(pushedAfterClaim.interaction.body).toBe("after-claim");

    const secondClaim = await withStage("second-claim", fetchWorker(
      new Request(provision.claim_url, {
        method: "POST",
        headers: {
          authorization: "Bearer test-internal-token",
          "x-hooktry-workspace-id": WORKSPACE_ID,
        },
      }),
    ));
    expect(secondClaim.status).toBe(410);

    socket.close(1000, "done");
    },
    35_000,
  );

  it("boots an existing viewer with one interaction snapshot", async () => {
    const provision = await createHook();

    for (const body of ["one", "two", "three"]) {
      const response = await fetchWorker(
        new Request(provision.hook_url, {
          method: "POST",
          body,
        }),
      );
      expect(response.status).toBe(200);
    }

    const viewerResponse = await fetchWorker(
      new Request(provision.view_url, {
        headers: { Upgrade: "websocket" },
      }),
    );
    expect(viewerResponse.status).toBe(101);
    const socket = viewerResponse.webSocket;
    if (!socket) {
      throw new Error("expected WebSocket response");
    }

    const inbox = jsonInbox(socket);
    socket.accept();

    const ready = await inbox.next();
    expect(ready.type).toBe("ready");
    expect(ready.exposure.request_count).toBe(3);

    const snapshot = await inbox.next();
    expect(snapshot.type).toBe("snapshot");
    expect(snapshot.interactions).toHaveLength(3);
    expect(snapshot.interactions.map((item: any) => item.sequence)).toEqual([1, 2, 3]);

    socket.close(1000, "done");
  });

  it("keeps a hibernatable viewer connected across Durable Object eviction", async () => {
    const provision = await createHook();
    const stub = bindings.EXPOSURES.getByName(provision.exposure_id);

    const viewerResponse = await stub.fetch(
      new Request("https://hooktry.internal/view", {
        headers: {
          Upgrade: "websocket",
          [INTERNAL_EXPOSURE_HEADER]: provision.exposure_id,
        },
      }),
    );
    expect(viewerResponse.status).toBe(101);

    const socket = viewerResponse.webSocket;
    if (!socket) {
      throw new Error("expected WebSocket response");
    }

    const inbox = jsonInbox(socket);
    socket.accept();

    const ready = await withStage("direct-viewer-ready", inbox.next());
    expect(ready.type).toBe("ready");
    expect(ready.exposure.exposure_id).toBe(provision.exposure_id);

    const initialSnapshot = await withStage("direct-viewer-snapshot", inbox.next());
    expect(initialSnapshot.type).toBe("snapshot");
    expect(initialSnapshot.interactions).toEqual([]);

    await withStage(
      "direct-evict",
      evictDurableObject(stub, { webSockets: "hibernate" }),
      5_000,
    );

    const captured = await withStage(
      "direct-capture-after-eviction",
      stub.fetch(
        new Request("https://hooktry.internal/hook/hk_test", {
          method: "POST",
          headers: {
            [INTERNAL_EXPOSURE_HEADER]: provision.exposure_id,
          },
          body: "after-eviction",
        }),
      ),
    );
    expect(captured.status).toBe(200);

    const pushed = await withStage("direct-push-after-eviction", inbox.next());
    expect(pushed.type).toBe("interaction");
    expect(pushed.interaction.sequence).toBe(1);
    expect(pushed.interaction.body).toBe("after-eviction");

    socket.close(1000, "done");
  }, 12_000);

  it("serves the shared React SPA for a direct view capability", async () => {
    const provision = await createHook();
    const response = await fetchWorker(
      new Request(provision.view_url, {
        headers: { accept: "text/html" },
      }),
    );

    expect(response.status).toBe(200);
    expect(response.headers.get("content-type")).toContain("text/html");
    expect(await response.text()).toContain('<div id="root"></div>');
  });

  it("does not allow the view capability to act as hook authority", async () => {
    const provision = await createHook();
    const viewToken = provision.view_url.split("/").pop();
    if (!viewToken) {
      throw new Error("missing view token");
    }

    const response = await fetchWorker(
      new Request(`https://hooktry.test/hook/${viewToken}`, {
        method: "POST",
        body: "must-not-route",
      }),
    );
    expect(response.status).toBe(404);
  });

  it("enforces three active ephemeral Hooks per principal", async () => {
    const first = await createHook();
    const headers = {
      "x-hooktry-anonymous-principal": first.anonymous_principal,
    };

    for (let index = 0; index < 2; index += 1) {
      const response = await fetchWorker(
        new Request("https://hooktry.test/api/v1/hooks", {
          method: "POST",
          headers,
        }),
      );
      expect(response.status).toBe(201);
    }

    const fourth = await fetchWorker(
      new Request("https://hooktry.test/api/v1/hooks", {
        method: "POST",
        headers,
      }),
    );
    expect(fourth.status).toBe(429);
    expect(await fourth.json()).toEqual({
      error: { code: "active_limit" },
    });
  });

  it("enforces request and retained-byte quotas from canonical metadata", async () => {
    const requestLimited = await createHook();
    await bindings.DB.prepare(
      "UPDATE anonymous_exposures SET request_count = ? WHERE exposure_id = ?",
    )
      .bind(ANONYMOUS_REQUEST_LIMIT, requestLimited.exposure_id)
      .run();

    const requestLimitResponse = await fetchWorker(
      new Request(requestLimited.hook_url, {
        method: "POST",
        body: "one-too-many",
      }),
    );
    expect(requestLimitResponse.status).toBe(429);
    expect(await requestLimitResponse.json()).toEqual({
      error: { code: "request_limit" },
    });

    const byteLimited = await createHook();
    await bindings.DB.prepare(
      "UPDATE anonymous_exposures SET retained_bytes = ? WHERE exposure_id = ?",
    )
      .bind(ANONYMOUS_MAX_RETAINED_BYTES, byteLimited.exposure_id)
      .run();

    const byteLimitResponse = await fetchWorker(
      new Request(byteLimited.hook_url, {
        method: "POST",
        body: "one-byte-too-many",
      }),
    );
    expect(byteLimitResponse.status).toBe(429);
    expect(await byteLimitResponse.json()).toEqual({
      error: { code: "byte_limit" },
    });
  });
});

async function createHook(): Promise<AnonymousProvision> {
  const response = await fetchWorker(
    new Request("https://hooktry.test/api/v1/hooks", {
      method: "POST",
    }),
  );
  expect(response.status).toBe(201);
  return (await response.json()) as AnonymousProvision;
}

function jsonInbox(socket: WebSocket): { next: () => Promise<any> } {
  const queued: any[] = [];
  const waiting: Array<{
    resolve: (value: any) => void;
    timer: ReturnType<typeof setTimeout>;
  }> = [];

  socket.addEventListener("message", (event) => {
    const value = JSON.parse(String(event.data));
    const waiter = waiting.shift();
    if (waiter) {
      clearTimeout(waiter.timer);
      waiter.resolve(value);
      return;
    }
    queued.push(value);
  });

  return {
    next(): Promise<any> {
      const value = queued.shift();
      if (value !== undefined) {
        return Promise.resolve(value);
      }

      return new Promise((resolve, reject) => {
        const waiter = {
          resolve,
          timer: setTimeout(() => {
            const index = waiting.indexOf(waiter);
            if (index >= 0) {
              waiting.splice(index, 1);
            }
            reject(new Error("timed out waiting for WebSocket message"));
          }, 2_000),
        };
        waiting.push(waiter);
      });
    },
  };
}

function withStage<T>(
  name: string,
  promise: Promise<T>,
  timeoutMs = 2_000,
): Promise<T> {
  return Promise.race([
    promise,
    new Promise<T>((_resolve, reject) => {
      setTimeout(
        () => reject(new Error(`stage timed out: ${name}`)),
        timeoutMs,
      );
    }),
  ]);
}
