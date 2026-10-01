const baseUrl = required("ORTYO_BASE_URL").replace(/\/$/, "");
const claimToken = required("ORTYO_CLAIM_INTERNAL_TOKEN");
const cloudflareToken = required("CLOUDFLARE_API_TOKEN");
const accountId = required("CLOUDFLARE_ACCOUNT_ID");
const databaseId = required("ORTYO_D1_DATABASE_ID");
const workspaceId = "0199a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5b";

let provision;
const interactions = [];

try {
  await waitForHealth();

  const app = await fetch(`${baseUrl}/`, {
    headers: { accept: "text/html" },
  });
  assert(app.ok, `web app failed: ${app.status}`);
  assert(
    (await app.text()).includes('<div id="root"></div>'),
    "shared React app shell is missing",
  );

  const create = await fetch(`${baseUrl}/api/v1/hooks`, { method: "POST" });
  if (create.status !== 201) {
    throw new Error(`create Hook failed: ${create.status} ${await create.text()}`);
  }
  provision = await create.json();

  const viewerPage = await fetch(provision.view_url, {
    headers: { accept: "text/html" },
  });
  assert(viewerPage.ok, `viewer SPA failed: ${viewerPage.status}`);
  assert(
    (await viewerPage.text()).includes('<div id="root"></div>'),
    "view capability did not resolve to the shared React app",
  );

  assert(provision.hook_url?.startsWith(baseUrl), "hook_url does not use deployed Worker");
  assert(provision.view_websocket_url?.startsWith("wss://"), "missing WebSocket viewer URL");
  assert(provision.claim_url?.startsWith(baseUrl), "claim_url does not use deployed Worker");

  const inbox = websocketInbox(provision.view_websocket_url);
  await inbox.opened;

  const ready = await inbox.next("ready");
  assert(ready.exposure?.exposure_id === provision.exposure_id, "viewer ready Exposure mismatch");

  const firstCapture = await fetch(`${provision.hook_url}/deploy1?phase=before-claim`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ proof: "deploy1-before-claim" }),
  });
  assert(firstCapture.ok, `first Hook capture failed: ${firstCapture.status}`);

  const firstInteraction = await inbox.next("interaction");
  interactions.push(firstInteraction.interaction);
  assert(firstInteraction.interaction.sequence === 1, "unexpected first Interaction sequence");
  assert(firstInteraction.interaction.path === "/deploy1", "unexpected captured path");

  const claim = await fetch(provision.claim_url, {
    method: "POST",
    headers: {
      authorization: `Bearer ${claimToken}`,
      "x-ortyo-workspace-id": workspaceId,
    },
  });
  if (!claim.ok) {
    throw new Error(`claim failed: ${claim.status} ${await claim.text()}`);
  }
  const claimed = await claim.json();
  assert(claimed.claimed === true, "claim response did not become persistent");
  assert(claimed.workspace_id === workspaceId, "claim workspace mismatch");

  const secondCapture = await fetch(`${provision.hook_url}/deploy1?phase=after-claim`, {
    method: "POST",
    body: "deploy1-after-claim",
  });
  assert(secondCapture.ok, `post-claim Hook capture failed: ${secondCapture.status}`);

  const secondInteraction = await inbox.next("interaction");
  interactions.push(secondInteraction.interaction);
  assert(secondInteraction.interaction.sequence === 2, "Hook URL did not continue after claim");

  const secondClaim = await fetch(provision.claim_url, {
    method: "POST",
    headers: {
      authorization: `Bearer ${claimToken}`,
      "x-ortyo-workspace-id": workspaceId,
    },
  });
  assert(secondClaim.status === 410, `claim capability was not single-use: ${secondClaim.status}`);

  inbox.close();
  console.log("WEB1 acceptance passed: app -> create -> React view -> live hook -> claim -> same hook");
} finally {
  if (provision?.exposure_id) {
    await cleanup(provision.exposure_id, interactions).catch((error) => {
      console.error("Smoke cleanup failed:", error);
      process.exitCode = 1;
    });
  }
}

async function waitForHealth() {
  const deadline = Date.now() + 30_000;
  let lastStatus = 0;

  while (Date.now() < deadline) {
    const response = await fetch(`${baseUrl}/healthz`).catch(() => null);
    if (response?.ok) {
      return;
    }
    lastStatus = response?.status ?? 0;
    await new Promise((resolve) => setTimeout(resolve, 1_000));
  }

  throw new Error(`healthz did not become ready within 30s; last status: ${lastStatus}`);
}

async function cleanup(exposureId, captured) {
  for (const interaction of captured) {
    if (!interaction?.interaction_id) continue;
    const key = `anonymous/${exposureId}/${interaction.interaction_id}`;
    const objectPath = key.split("/").map(encodeURIComponent).join("/");

    await cf(
      `/accounts/${accountId}/r2/buckets/ortyo-payloads/objects/${objectPath}`,
      { method: "DELETE" },
      true,
    );

    const listed = await cf(
      `/accounts/${accountId}/r2/buckets/ortyo-payloads/objects?prefix=${encodeURIComponent(key)}`,
    );
    assert(
      !(listed.result ?? []).some((object) => object.key === key),
      `R2 smoke body still exists after delete: ${key}`,
    );
  }

  await d1("DELETE FROM anonymous_interactions WHERE exposure_id = ?", [exposureId]);
  await d1("DELETE FROM anonymous_exposures WHERE exposure_id = ?", [exposureId]);
  console.log("DEPLOY1 smoke resources cleaned up");
}

async function d1(sql, params) {
  const payload = await cf(
    `/accounts/${accountId}/d1/database/${databaseId}/query`,
    {
      method: "POST",
      body: { sql, params },
    },
  );
  return payload.result;
}

async function cf(path, options = {}, allowNotFound = false) {
  const response = await fetch(`https://api.cloudflare.com/client/v4${path}`, {
    method: options.method ?? "GET",
    headers: {
      authorization: `Bearer ${cloudflareToken}`,
      ...(options.body ? { "content-type": "application/json" } : {}),
    },
    body: options.body ? JSON.stringify(options.body) : undefined,
  });

  const payload = await response.json().catch(() => ({}));
  if (allowNotFound && response.status === 404) {
    return payload;
  }
  if (!response.ok || payload.success === false) {
    throw new Error(`Cloudflare API ${response.status}: ${JSON.stringify(payload.errors ?? payload)}`);
  }
  return payload;
}

function websocketInbox(url) {
  const socket = new WebSocket(url);
  const queued = [];
  const waiters = [];

  const opened = new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error("WebSocket open timed out")), 10_000);
    socket.addEventListener("open", () => {
      clearTimeout(timeout);
      resolve();
    }, { once: true });
    socket.addEventListener("error", () => {
      clearTimeout(timeout);
      reject(new Error("WebSocket connection failed"));
    }, { once: true });
  });

  socket.addEventListener("message", (event) => {
    const value = JSON.parse(String(event.data));
    const waiterIndex = waiters.findIndex((waiter) => waiter.type === value.type);
    if (waiterIndex >= 0) {
      const [waiter] = waiters.splice(waiterIndex, 1);
      clearTimeout(waiter.timer);
      waiter.resolve(value);
    } else {
      queued.push(value);
    }
  });

  return {
    opened,
    next(type) {
      const index = queued.findIndex((value) => value.type === type);
      if (index >= 0) {
        return Promise.resolve(queued.splice(index, 1)[0]);
      }
      return new Promise((resolve, reject) => {
        const waiter = {
          type,
          resolve,
          timer: setTimeout(() => {
            const index = waiters.indexOf(waiter);
            if (index >= 0) waiters.splice(index, 1);
            reject(new Error(`WebSocket frame timed out: ${type}`));
          }, 10_000),
        };
        waiters.push(waiter);
      });
    },
    close() {
      socket.close(1000, "deploy1-complete");
    },
  };
}

function required(name) {
  const value = process.env[name]?.trim();
  if (!value) throw new Error(`${name} is required`);
  return value;
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}
