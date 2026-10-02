import { createHash, randomUUID } from "node:crypto";

const baseUrl = required("HOOKTRY_BASE_URL").replace(/\/$/, "");
const claimToken = required("HOOKTRY_CLAIM_INTERNAL_TOKEN");
const cloudflareToken = required("CLOUDFLARE_API_TOKEN");
const accountId = required("CLOUDFLARE_ACCOUNT_ID");
const databaseId = required("HOOKTRY_D1_DATABASE_ID");
const workspaceId = "0199a2b3-c4d5-7e6f-8a9b-0c1d2e3f4a5b";
const expectedReleaseSha = process.env.HOOKTRY_EXPECTED_RELEASE_SHA?.trim() || null;
const expectGitHubAuth = process.env.HOOKTRY_EXPECT_GITHUB_AUTH === "1";
const usageIngestToken = process.env.HOOKTRY_USAGE_INGEST_TOKEN?.trim() || null;

let provision;
let oauthStateDigest = null;
let usageEventId = null;
const interactions = [];

try {
  await waitForHealth();
  if (expectGitHubAuth) {
    oauthStateDigest = await verifyGitHubAuthStart();
  }
  if (usageIngestToken) {
    usageEventId = randomUUID();
    await verifyUsageIngest(usageEventId);
  }

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
      "x-hooktry-workspace-id": workspaceId,
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
      "x-hooktry-workspace-id": workspaceId,
    },
  });
  assert(secondClaim.status === 410, `claim capability was not single-use: ${secondClaim.status}`);

  inbox.close();
  console.log("WEB1 acceptance passed: app -> create -> React view -> live hook -> claim -> same hook");
} finally {
  if (usageEventId) {
    await d1(
      "DELETE FROM usage_events WHERE event_id = ?",
      [usageEventId],
    ).catch((error) => {
      console.error("Usage smoke cleanup failed:", error);
      process.exitCode = 1;
    });
  }

  if (oauthStateDigest) {
    await d1(
      "DELETE FROM auth_oauth_states WHERE state_digest = ?",
      [oauthStateDigest],
    ).catch((error) => {
      console.error("OAuth smoke cleanup failed:", error);
      process.exitCode = 1;
    });
  }

  if (provision?.exposure_id) {
    await cleanup(provision.exposure_id, interactions).catch((error) => {
      console.error("Smoke cleanup failed:", error);
      process.exitCode = 1;
    });
  }
}

async function verifyUsageIngest(eventId) {
  if (!usageIngestToken) {
    throw new Error("usage ingest token missing");
  }

  const event = {
    schema_version: 1,
    event_id: eventId,
    event: "scenario_run_completed",
    occurred_at_unix_ms: Date.now(),
    passed: false,
    command_success: true,
    outcome_passed: false,
    check_count: 1,
    features: {
      contract_count: 1,
      exact_cardinality: true,
      ranged_cardinality: false,
      ordering: false,
      observation_horizon: true,
      settle_window: true,
      context_match: true,
      idempotency_context: true,
      duplicate_guard: true,
    },
  };

  const response = await fetch(`${baseUrl}/api/v1/usage-events`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${usageIngestToken}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(event),
  });
  assert(response.status === 202, `usage ingest failed: ${response.status}`);
  const accepted = await response.json();
  assert(accepted.accepted === true, "usage ingest did not accept event");
  assert(accepted.stored === true, "usage ingest did not persist new event");

  const stored = await d1(
    `SELECT event_type, passed, command_success, outcome_passed,
            contract_count, exact_cardinality, ordering_enabled,
            settle_window, idempotency_context, duplicate_guard
     FROM usage_events
     WHERE event_id = ?`,
    [eventId],
  );
  const row = stored?.[0]?.results?.[0];
  assert(row?.event_type === "scenario_run_completed", "usage event type mismatch");
  assert(row?.passed === 0, "usage result mismatch");
  assert(row?.command_success === 1, "usage command result mismatch");
  assert(row?.outcome_passed === 0, "usage outcome result mismatch");
  assert(row?.contract_count === 1, "usage contract count mismatch");
  assert(row?.exact_cardinality === 1, "usage cardinality shape mismatch");
  assert(row?.ordering_enabled === 0, "usage ordering shape mismatch");
  assert(row?.settle_window === 1, "usage settle shape mismatch");
  assert(row?.idempotency_context === 1, "usage idempotency shape mismatch");
  assert(row?.duplicate_guard === 1, "usage duplicate guard mismatch");

  console.log("FIRST-PARTY-PROOF acceptance passed: privacy-safe usage event -> D1");
}

async function verifyGitHubAuthStart() {
  const response = await fetch(
    `${baseUrl}/api/v1/auth/github/start?return_to=${encodeURIComponent("/")}`,
    { redirect: "manual" },
  );
  assert(response.status === 302, `GitHub auth start failed: ${response.status}`);

  const location = response.headers.get("location");
  assert(location, "GitHub auth start did not return Location");
  const authorize = new URL(location);
  assert(authorize.origin === "https://github.com", "GitHub auth origin mismatch");
  assert(authorize.pathname === "/login/oauth/authorize", "GitHub auth path mismatch");
  assert(!authorize.searchParams.has("scope"), "GitHub auth unexpectedly requests OAuth scopes");
  assert(
    authorize.searchParams.get("code_challenge_method") === "S256",
    "GitHub auth PKCE method mismatch",
  );
  assert(
    /^[A-Za-z0-9_-]{43}$/.test(authorize.searchParams.get("code_challenge") ?? ""),
    "GitHub auth PKCE challenge missing",
  );

  const state = authorize.searchParams.get("state");
  assert(state, "GitHub auth state missing");
  const digest = createHash("sha256").update(state).digest("hex");
  const stored = await d1(
    "SELECT COUNT(*) AS count FROM auth_oauth_states WHERE state_digest = ?",
    [digest],
  );
  assert(
    stored?.[0]?.results?.[0]?.count === 1,
    "GitHub auth state was not durably stored",
  );

  console.log("AUTH1 acceptance passed: GitHub redirect + state + PKCE");
  return digest;
}

async function waitForHealth() {
  const deadline = Date.now() + 30_000;
  let consecutive = 0;
  let lastStatus = 0;
  let lastRevision = null;
  let lastGitHubAuthConfigured = false;
  let lastUsageIngestConfigured = false;

  while (Date.now() < deadline) {
    const response = await fetch(`${baseUrl}/healthz`).catch(() => null);
    lastStatus = response?.status ?? 0;

    if (response?.ok) {
      const payload = await response.json().catch(() => null);
      lastRevision = payload?.revision ?? null;
      lastGitHubAuthConfigured = payload?.github_auth_configured === true;
      lastUsageIngestConfigured = payload?.usage_ingest_configured === true;

      const revisionReady =
        !expectedReleaseSha || lastRevision === expectedReleaseSha;
      const authReady = !expectGitHubAuth || lastGitHubAuthConfigured;
      const usageReady = !usageIngestToken || lastUsageIngestConfigured;

      if (revisionReady && authReady && usageReady) {
        consecutive += 1;
        if (consecutive >= 3) {
          return;
        }
      } else {
        consecutive = 0;
      }
    } else {
      consecutive = 0;
    }

    await new Promise((resolve) => setTimeout(resolve, 500));
  }

  throw new Error(
    `healthz did not converge within 30s; last status: ${lastStatus}; expected revision: ${expectedReleaseSha ?? "any"}; last revision: ${lastRevision ?? "none"}; github auth configured: ${lastGitHubAuthConfigured}; usage ingest configured: ${lastUsageIngestConfigured}`,
  );
}

async function cleanup(exposureId, captured) {
  for (const interaction of captured) {
    if (!interaction?.interaction_id) continue;
    const key = `anonymous/${exposureId}/${interaction.interaction_id}`;
    const objectPath = key.split("/").map(encodeURIComponent).join("/");

    await cf(
      `/accounts/${accountId}/r2/buckets/hooktry-payloads/objects/${objectPath}`,
      { method: "DELETE" },
      true,
    );

    const listed = await cf(
      `/accounts/${accountId}/r2/buckets/hooktry-payloads/objects?prefix=${encodeURIComponent(key)}`,
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
