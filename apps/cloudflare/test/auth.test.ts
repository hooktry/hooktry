import { env } from "cloudflare:workers";
import { beforeEach, describe, expect, it } from "vitest";

import {
  finishGitHubOAuth,
  sessionResponse,
  startGitHubOAuth,
} from "../src/auth";
import worker from "../src/index";
import type { AnonymousProvision, Env } from "../src/types";

const bindings = env as unknown as Env;

beforeEach(async () => {
  await bindings.DB.batch([
    bindings.DB.prepare("DELETE FROM auth_sessions"),
    bindings.DB.prepare("DELETE FROM auth_oauth_states"),
    bindings.DB.prepare("DELETE FROM auth_workspaces"),
    bindings.DB.prepare("DELETE FROM auth_users"),
  ]);
});

describe("AUTH1 GitHub claim flow", () => {
  it("starts a state-bound no-repository GitHub authorization flow", async () => {
    const returnTo =
      "/view/vw_12345678901234567890123456789012?claim=1";
    const response = await startGitHubOAuth(
      new Request(
        `https://ortyo.test/api/v1/auth/github/start?return_to=${encodeURIComponent(returnTo)}`,
      ),
      bindings,
    );

    expect(response.status).toBe(302);
    const location = new URL(requiredHeader(response, "location"));
    expect(location.origin).toBe("https://github.com");
    expect(location.pathname).toBe("/login/oauth/authorize");
    expect(location.searchParams.get("client_id")).toBe("test-github-client");
    expect(location.searchParams.get("scope")).toBe("read:user");
    expect(location.searchParams.get("redirect_uri")).toBe(
      "https://ortyo.test/api/v1/auth/github/callback",
    );

    const state = location.searchParams.get("state");
    expect(state).toMatch(/^oauth_[A-Za-z0-9_-]+$/);
    expect(location.searchParams.get("code_challenge_method")).toBe("S256");
    expect(location.searchParams.get("code_challenge")).toMatch(
      /^[A-Za-z0-9_-]{43}$/,
    );

    const cookie = requiredHeader(response, "set-cookie");
    expect(cookie).toContain(`ortyo_oauth_state=${state}`);
    expect(cookie).toContain("ortyo_oauth_pkce=");
    expect(cookie).toContain("HttpOnly");
    expect(cookie).toContain("SameSite=Lax");
    expect(cookie).toContain("Secure");

    const stored = await bindings.DB.prepare(
      "SELECT return_to, COUNT(*) AS count FROM auth_oauth_states",
    ).first<{ return_to: string; count: number }>();
    expect(stored).toEqual({ return_to: returnTo, count: 1 });
  });

  it("rejects cross-origin OAuth return targets", async () => {
    const response = await worker.fetch(
      new Request(
        "https://ortyo.test/api/v1/auth/github/start?return_to=https%3A%2F%2Fevil.example%2Fsteal",
      ),
      bindings,
    );

    expect(response.status).toBe(400);
    expect(await response.json()).toEqual({
      error: { code: "invalid_return_to" },
    });
  });

  it("creates one durable identity/workspace, keeps the GitHub token ephemeral, and publishes a session", async () => {
    const first = await login("/view/vw_12345678901234567890123456789012?claim=1");

    expect(first.response.status).toBe(302);
    expect(requiredHeader(first.response, "location")).toBe(
      "https://ortyo.test/view/vw_12345678901234567890123456789012?claim=1",
    );

    const firstSession = await sessionResponse(
      new Request("https://ortyo.test/api/v1/session", {
        headers: { cookie: first.sessionCookie },
      }),
      bindings,
    );
    expect(firstSession.status).toBe(200);
    expect(await firstSession.json()).toMatchObject({
      authenticated: true,
      user: {
        github_user_id: "42",
        github_login: "octocat",
      },
      workspace: {
        kind: "personal",
      },
    });

    const second = await login("/");
    expect(second.response.status).toBe(302);

    const counts = await bindings.DB.prepare(
      `SELECT
       (SELECT COUNT(*) FROM auth_users) AS users,
       (SELECT COUNT(*) FROM auth_workspaces) AS workspaces,
       (SELECT COUNT(*) FROM auth_sessions) AS sessions,
       (SELECT COUNT(*) FROM auth_oauth_states) AS oauth_states`,
    ).first<{
      users: number;
      workspaces: number;
      sessions: number;
      oauth_states: number;
    }>();

    expect(counts).toEqual({
      users: 1,
      workspaces: 1,
      sessions: 2,
      oauth_states: 0,
    });

    const persisted = await bindings.DB.prepare(
      `SELECT
       u.github_login,
       u.github_user_id,
       w.slug,
       s.session_digest
     FROM auth_users u
     JOIN auth_workspaces w ON w.owner_user_id = u.user_id
     JOIN auth_sessions s ON s.user_id = u.user_id
     LIMIT 1`,
    ).first<Record<string, unknown>>();

    expect(JSON.stringify(persisted)).not.toContain("gho_ephemeral_test_token");
    expect(JSON.stringify(persisted)).not.toContain(
      first.sessionCookie.split("=")[1],
    );
  });

  it("consumes OAuth state exactly once", async () => {
    const start = await startGitHubOAuth(
      new Request(
        "https://ortyo.test/api/v1/auth/github/start?return_to=%2F",
      ),
      bindings,
    );
    const authorize = new URL(requiredHeader(start, "location"));
    const state = authorize.searchParams.get("state");
    if (!state) throw new Error("missing OAuth state");

    const cookies = requiredHeader(start, "set-cookie");
    const pkce = cookies.match(/ortyo_oauth_pkce=([^;,]+)/)?.[1];
    if (!pkce) throw new Error("missing OAuth PKCE cookie");

    const callback = new Request(
      `https://ortyo.test/api/v1/auth/github/callback?code=test-code&state=${encodeURIComponent(state)}`,
      {
        headers: {
          cookie: `ortyo_oauth_state=${state}; ortyo_oauth_pkce=${pkce}`,
        },
      },
    );

    const first = await finishGitHubOAuth(
      callback.clone(),
      bindings,
      fakeGitHubFetch,
    );
    expect(first.status).toBe(302);

    await expect(
      finishGitHubOAuth(callback, bindings, fakeGitHubFetch),
    ).rejects.toMatchObject({
      status: 400,
      code: "oauth_state_invalid",
    });
  });

  it("claims an existing anonymous Hook with the authenticated personal workspace and preserves the Hook", async () => {
    const create = await worker.fetch(
      new Request("https://ortyo.test/api/v1/hooks", {
        method: "POST",
      }),
      bindings,
    );
    expect(create.status).toBe(201);
    const provision = await create.json() as AnonymousProvision;

    const beforeClaim = await worker.fetch(
      new Request(`${provision.hook_url}/before-claim`, {
        method: "POST",
        body: "before",
      }),
      bindings,
    );
    expect(beforeClaim.status).toBe(200);

    const auth = await login(
      `${new URL(provision.view_url).pathname}?claim=1`,
    );

    const session = await sessionResponse(
      new Request("https://ortyo.test/api/v1/session", {
        headers: { cookie: auth.sessionCookie },
      }),
      bindings,
    );
    const sessionBody = await session.json() as {
      authenticated: true;
      workspace: { workspace_id: string };
    };

    const claim = await worker.fetch(
      new Request(provision.claim_url, {
        method: "POST",
        headers: { cookie: auth.sessionCookie },
      }),
      bindings,
    );
    expect(claim.status).toBe(200);
    expect(await claim.json()).toMatchObject({
      exposure_id: provision.exposure_id,
      workspace_id: sessionBody.workspace.workspace_id,
      claimed: true,
      request_count: 1,
    });

    const afterClaim = await worker.fetch(
      new Request(`${provision.hook_url}/after-claim`, {
        method: "POST",
        body: "after",
      }),
      bindings,
    );
    expect(afterClaim.status).toBe(200);
    expect(await afterClaim.json()).toMatchObject({
      ok: true,
      sequence: 2,
    });

    const stored = await bindings.DB.prepare(
      `SELECT workspace_id, claim_capability_digest, request_count
       FROM anonymous_exposures
       WHERE exposure_id = ?`,
    )
      .bind(provision.exposure_id)
      .first<{
        workspace_id: string;
        claim_capability_digest: string | null;
        request_count: number;
      }>();

    expect(stored).toEqual({
      workspace_id: sessionBody.workspace.workspace_id,
      claim_capability_digest: null,
      request_count: 2,
    });

    const history = await bindings.DB.prepare(
      "SELECT COUNT(*) AS count FROM anonymous_interactions WHERE exposure_id = ?",
    )
      .bind(provision.exposure_id)
      .first<{ count: number }>();
    expect(history?.count).toBe(2);
  });
});

async function login(returnTo: string): Promise<{
  response: Response;
  sessionCookie: string;
}> {
  const start = await startGitHubOAuth(
    new Request(
      `https://ortyo.test/api/v1/auth/github/start?return_to=${encodeURIComponent(returnTo)}`,
    ),
    bindings,
  );
  const authorize = new URL(requiredHeader(start, "location"));
  const state = authorize.searchParams.get("state");
  if (!state) throw new Error("missing OAuth state");

  const startCookies = requiredHeader(start, "set-cookie");
  const pkce = startCookies.match(/ortyo_oauth_pkce=([^;,]+)/)?.[1];
  if (!pkce) throw new Error("missing OAuth PKCE cookie");

  const response = await finishGitHubOAuth(
    new Request(
      `https://ortyo.test/api/v1/auth/github/callback?code=test-code&state=${encodeURIComponent(state)}`,
      {
        headers: {
          cookie: `ortyo_oauth_state=${state}; ortyo_oauth_pkce=${pkce}`,
        },
      },
    ),
    bindings,
    fakeGitHubFetch,
  );

  const cookies = requiredHeader(response, "set-cookie");
  const match = cookies.match(/ortyo_session=([^;,]+)/);
  if (!match) throw new Error(`missing session cookie: ${cookies}`);

  return {
    response,
    sessionCookie: `ortyo_session=${match[1]}`,
  };
}

const fakeGitHubFetch: typeof fetch = async (input, init) => {
  const url = typeof input === "string" ? input : input.toString();

  if (url === "https://github.com/login/oauth/access_token") {
    expect(init?.method).toBe("POST");
    expect(String(init?.body)).toContain("client_id=test-github-client");
    expect(String(init?.body)).toContain("client_secret=test-github-secret");
    expect(String(init?.body)).toMatch(/code_verifier=[A-Za-z0-9_-]{43}/);
    return Response.json({
      access_token: "gho_ephemeral_test_token",
      scope: "read:user",
      token_type: "bearer",
    });
  }

  if (url === "https://api.github.com/user") {
    expect(new Headers(init?.headers).get("authorization")).toBe(
      "Bearer gho_ephemeral_test_token",
    );
    return Response.json({
      id: 42,
      login: "octocat",
      avatar_url: "https://avatars.example/octocat.png",
    });
  }

  throw new Error(`unexpected GitHub fetch: ${url}`);
};

function requiredHeader(response: Response, name: string): string {
  const value = response.headers.get(name);
  if (!value) throw new Error(`missing response header: ${name}`);
  return value;
}
