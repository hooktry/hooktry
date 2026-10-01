import { env } from "cloudflare:workers";
import { evictDurableObject } from "cloudflare:test";
import { describe, expect, it } from "vitest";

import {
  ANONYMOUS_MAX_RETAINED_BYTES,
  ANONYMOUS_REQUEST_LIMIT,
} from "../src/core";
import worker from "../src/index";
import type { AnonymousProvision, Env } from "../src/types";

const bindings = env as unknown as Env;
const WORKSPACE_ID = "0199a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5b";

function fetchWorker(request: Request): Promise<Response> {
  return worker.fetch(request, bindings);
}

describe("CF1 anonymous Exposure conformance", () => {
  it("creates, pushes, survives DO eviction, claims, and preserves the hook URL", async () => {
    const provision = await createAnonymous();

    expect(provision.hook_url).toMatch(/\/hook\/hk_[A-Za-z0-9_-]{32}$/);
    expect(provision.view_url).toMatch(/\/view\/vw_[A-Za-z0-9_-]{32}$/);
    expect(provision.claim_url).toMatch(/\/claim\/cl_[A-Za-z0-9_-]{32}$/);
    expect(provision.expires_at_unix_seconds).toBeTypeOf("number");

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
    expect(ready.exposure.exposure_id).toBe(provision.exposure_id);

    const first = await fetchWorker(
      new Request(`${provision.hook_url}/stripe?delivery=42`, {
        method: "POST",
        headers: {
          "content-type": "application/json",
          "stripe-signature": "proof",
        },
        body: JSON.stringify({ type: "checkout.session.completed" }),
      }),
    );
    expect(first.status).toBe(200);

    const pushed = await inbox.next();
    expect(pushed.type).toBe("interaction");
    expect(pushed.interaction.sequence).toBe(1);
    expect(pushed.interaction.path).toBe("/stripe");
    expect(pushed.interaction.query).toBe("delivery=42");
    expect(pushed.interaction.body_encoding).toBe("utf8");

    const stub = bindings.EXPOSURES.getByName(provision.exposure_id);
    await evictDurableObject(stub);

    const afterEviction = await fetchWorker(
      new Request(provision.hook_url, {
        method: "POST",
        body: "after-eviction",
      }),
    );
    expect(afterEviction.status).toBe(200);
    const pushedAfterEviction = await inbox.next();
    expect(pushedAfterEviction.interaction.sequence).toBe(2);
    expect(pushedAfterEviction.interaction.body).toBe("after-eviction");

    const claimed = await fetchWorker(
      new Request(provision.claim_url, {
        method: "POST",
        headers: {
          authorization: "Bearer test-internal-token",
          "x-ortyo-workspace-id": WORKSPACE_ID,
        },
      }),
    );
    expect(claimed.status).toBe(200);
    const claimedBody = (await claimed.json()) as Record<string, unknown>;
    expect(claimedBody.claimed).toBe(true);
    expect(claimedBody.workspace_id).toBe(WORKSPACE_ID);
    expect(claimedBody.expires_at_unix_seconds).toBeUndefined();

    const afterClaim = await fetchWorker(
      new Request(provision.hook_url, {
        method: "POST",
        body: "after-claim",
      }),
    );
    expect(afterClaim.status).toBe(200);
    const pushedAfterClaim = await inbox.next();
    expect(pushedAfterClaim.interaction.sequence).toBe(3);
    expect(pushedAfterClaim.interaction.body).toBe("after-claim");

    const secondClaim = await fetchWorker(
      new Request(provision.claim_url, {
        method: "POST",
        headers: {
          authorization: "Bearer test-internal-token",
          "x-ortyo-workspace-id": WORKSPACE_ID,
        },
      }),
    );
    expect(secondClaim.status).toBe(410);

    socket.close(1000, "done");
  });

  it("does not allow the view capability to act as hook authority", async () => {
    const provision = await createAnonymous();
    const viewToken = provision.view_url.split("/").pop();
    if (!viewToken) {
      throw new Error("missing view token");
    }

    const response = await fetchWorker(
      new Request(`https://ortyo.test/hook/${viewToken}`, {
        method: "POST",
        body: "must-not-route",
      }),
    );
    expect(response.status).toBe(404);
  });

  it("enforces three active anonymous Exposures per principal", async () => {
    const first = await createAnonymous();
    const headers = {
      "x-ortyo-anonymous-principal": first.anonymous_principal,
    };

    for (let index = 0; index < 2; index += 1) {
      const response = await fetchWorker(
        new Request("https://ortyo.test/_ortyo/anonymous/exposures", {
          method: "POST",
          headers,
        }),
      );
      expect(response.status).toBe(201);
    }

    const fourth = await fetchWorker(
      new Request("https://ortyo.test/_ortyo/anonymous/exposures", {
        method: "POST",
        headers,
      }),
    );
    expect(fourth.status).toBe(429);
  });

  it("enforces request and retained-byte quotas from canonical metadata", async () => {
    const requestLimited = await createAnonymous();
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

    const byteLimited = await createAnonymous();
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

async function createAnonymous(): Promise<AnonymousProvision> {
  const response = await fetchWorker(
    new Request("https://ortyo.test/_ortyo/anonymous/exposures", {
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
