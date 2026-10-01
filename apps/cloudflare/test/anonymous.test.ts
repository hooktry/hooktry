import { env } from "cloudflare:workers";
import { evictDurableObject } from "cloudflare:test";
import { describe, expect, it } from "vitest";

import {
  ANONYMOUS_MAX_RETAINED_BYTES,
  ANONYMOUS_REQUEST_LIMIT,
} from "../src/core";
import worker from "../src/index";
import type { AnonymousProvision, Env } from "../src/types";

const testEnv = env as unknown as Env;

function fetchWorker(request: Request): Promise<Response> {
  return worker.fetch(request, testEnv);
}

const WORKSPACE_ID = "0199a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5b";

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
    socket.accept();

    const ready = await nextJson(socket);
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

    const pushed = await nextJson(socket);
    expect(pushed.type).toBe("interaction");
    expect(pushed.interaction.sequence).toBe(1);
    expect(pushed.interaction.path).toBe("/stripe");
    expect(pushed.interaction.query).toBe("delivery=42");
    expect(pushed.interaction.body_encoding).toBe("utf8");

    const stub = testEnv.EXPOSURES.getByName(provision.exposure_id);
    await evictDurableObject(stub);

    const afterEviction = await fetchWorker(
      new Request(provision.hook_url, {
        method: "POST",
        body: "after-eviction",
      }),
    );
    expect(afterEviction.status).toBe(200);
    const pushedAfterEviction = await nextJson(socket);
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
    const pushedAfterClaim = await nextJson(socket);
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

    expect(
      (
        await fetchWorker(
          new Request("https://ortyo.test/_ortyo/anonymous/exposures", {
            method: "POST",
            headers,
          }),
        )
      ).status,
    ).toBe(201);

    expect(
      (
        await fetchWorker(
          new Request("https://ortyo.test/_ortyo/anonymous/exposures", {
            method: "POST",
            headers,
          }),
        )
      ).status,
    ).toBe(201);

    expect(
      (
        await fetchWorker(
          new Request("https://ortyo.test/_ortyo/anonymous/exposures", {
            method: "POST",
            headers,
          }),
        )
      ).status,
    ).toBe(429);
  });

  it("enforces request and retained-byte quotas from canonical metadata", async () => {
    const requestLimited = await createAnonymous();
    await testEnv.DB.prepare(
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
    await testEnv.DB.prepare(
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

function nextJson(socket: WebSocket): Promise<any> {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(
      () => reject(new Error("timed out waiting for WebSocket message")),
      2_000,
    );
    socket.addEventListener(
      "message",
      (event) => {
        clearTimeout(timeout);
        try {
          resolve(JSON.parse(String(event.data)));
        } catch (error) {
          reject(error);
        }
      },
      { once: true },
    );
  });
}
