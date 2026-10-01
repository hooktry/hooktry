import {
  AdapterError,
  json,
  sha256Hex,
} from "./core";
import {
  consumeOAuthState,
  createOAuthState,
  createSession,
  deleteSession,
  findSession,
  upsertGitHubIdentity,
} from "./auth-repository";
import type {
  AuthSession,
  Env,
  GitHubIdentity,
  SessionView,
} from "./types";

const OAUTH_STATE_COOKIE = "ortyo_oauth_state";
const SESSION_COOKIE = "ortyo_session";
const OAUTH_STATE_TTL_SECONDS = 10 * 60;
const SESSION_TTL_SECONDS = 30 * 24 * 60 * 60;

export async function startGitHubOAuth(
  request: Request,
  env: Env,
): Promise<Response> {
  if (!env.GITHUB_CLIENT_ID) {
    throw new AdapterError(503, "github_auth_unconfigured");
  }

  const requestUrl = new URL(request.url);
  const returnTo = safeReturnTo(
    requestUrl.searchParams.get("return_to") ?? "/",
  );
  const state = randomOpaque("oauth_");
  const now = Math.floor(Date.now() / 1000);

  await createOAuthState(
    env,
    await sha256Hex(state),
    returnTo,
    now,
    now + OAUTH_STATE_TTL_SECONDS,
  );

  const redirectUri = githubCallbackUrl(requestUrl);
  const authorize = new URL("https://github.com/login/oauth/authorize");
  authorize.searchParams.set("client_id", env.GITHUB_CLIENT_ID);
  authorize.searchParams.set("redirect_uri", redirectUri);
  authorize.searchParams.set("scope", "read:user");
  authorize.searchParams.set("state", state);

  const response = new Response(null, {
    status: 302,
    headers: { location: authorize.toString() },
  });
  response.headers.append(
    "set-cookie",
    cookie(
      OAUTH_STATE_COOKIE,
      state,
      OAUTH_STATE_TTL_SECONDS,
      requestUrl,
      "/api/v1/auth/github",
    ),
  );
  response.headers.set("cache-control", "no-store");
  return response;
}

export async function finishGitHubOAuth(
  request: Request,
  env: Env,
  githubFetch: typeof fetch = fetch,
): Promise<Response> {
  if (!env.GITHUB_CLIENT_ID || !env.GITHUB_CLIENT_SECRET) {
    throw new AdapterError(503, "github_auth_unconfigured");
  }

  const url = new URL(request.url);
  if (url.searchParams.get("error")) {
    throw new AdapterError(401, "github_auth_denied");
  }

  const code = url.searchParams.get("code");
  const state = url.searchParams.get("state");
  const cookieState = cookieValue(request, OAUTH_STATE_COOKIE);
  if (!code || !state || !cookieState || state !== cookieState) {
    throw new AdapterError(400, "oauth_state_invalid");
  }

  const now = Math.floor(Date.now() / 1000);
  const returnTo = await consumeOAuthState(
    env,
    await sha256Hex(state),
    now,
  );

  const identity = await githubIdentity(
    code,
    githubCallbackUrl(url),
    env,
    githubFetch,
  );
  const { user, workspace } = await upsertGitHubIdentity(
    env,
    identity,
    now,
  );

  const session = randomOpaque("sess_");
  await createSession(
    env,
    await sha256Hex(session),
    user.user_id,
    workspace.workspace_id,
    now,
    now + SESSION_TTL_SECONDS,
  );

  const destination = new URL(returnTo, url.origin);
  const response = new Response(null, {
    status: 302,
    headers: { location: destination.toString() },
  });
  response.headers.append(
    "set-cookie",
    cookie(SESSION_COOKIE, session, SESSION_TTL_SECONDS, url),
  );
  response.headers.append(
    "set-cookie",
    clearCookie(OAUTH_STATE_COOKIE, url, "/api/v1/auth/github"),
  );
  response.headers.set("cache-control", "no-store");
  return response;
}

export async function sessionResponse(
  request: Request,
  env: Env,
): Promise<Response> {
  const session = await sessionForRequest(request, env);
  const response = session
    ? json(toSessionView(session))
    : json({ authenticated: false });
  response.headers.set("cache-control", "no-store");
  return response;
}

export async function logout(
  request: Request,
  env: Env,
): Promise<Response> {
  const raw = cookieValue(request, SESSION_COOKIE);
  if (raw) {
    await deleteSession(env, await sha256Hex(raw));
  }

  const url = new URL(request.url);
  const response = new Response(null, { status: 204 });
  response.headers.append(
    "set-cookie",
    clearCookie(SESSION_COOKIE, url, "/"),
  );
  response.headers.set("cache-control", "no-store");
  return response;
}

export async function sessionForRequest(
  request: Request,
  env: Env,
): Promise<AuthSession | null> {
  const raw = cookieValue(request, SESSION_COOKIE);
  if (!raw) {
    return null;
  }
  return findSession(
    env,
    await sha256Hex(raw),
    Math.floor(Date.now() / 1000),
  );
}

export async function githubIdentity(
  code: string,
  redirectUri: string,
  env: Env,
  githubFetch: typeof fetch = fetch,
): Promise<GitHubIdentity> {
  if (!env.GITHUB_CLIENT_ID || !env.GITHUB_CLIENT_SECRET) {
    throw new AdapterError(503, "github_auth_unconfigured");
  }

  const tokenResponse = await githubFetch(
    "https://github.com/login/oauth/access_token",
    {
      method: "POST",
      headers: {
        accept: "application/json",
        "content-type": "application/x-www-form-urlencoded",
      },
      body: new URLSearchParams({
        client_id: env.GITHUB_CLIENT_ID,
        client_secret: env.GITHUB_CLIENT_SECRET,
        code,
        redirect_uri: redirectUri,
      }).toString(),
    },
  );

  const tokenPayload = await tokenResponse.json() as {
    access_token?: string;
    error?: string;
  };
  if (!tokenResponse.ok || !tokenPayload.access_token) {
    throw new AdapterError(401, "github_token_exchange_failed");
  }

  const userResponse = await githubFetch("https://api.github.com/user", {
    headers: {
      accept: "application/vnd.github+json",
      authorization: `Bearer ${tokenPayload.access_token}`,
      "user-agent": "ortyo-auth",
    },
  });

  const user = await userResponse.json() as {
    id?: number | string;
    login?: string;
    avatar_url?: string | null;
  };
  if (
    !userResponse.ok ||
    user.id === undefined ||
    typeof user.login !== "string" ||
    user.login.length === 0
  ) {
    throw new AdapterError(401, "github_identity_failed");
  }

  return {
    github_user_id: String(user.id),
    github_login: user.login,
    github_avatar_url:
      typeof user.avatar_url === "string" ? user.avatar_url : null,
  };
}

export function safeReturnTo(value: string): string {
  const base = new URL("https://ortyo.invalid/");
  const parsed = new URL(value, base);
  if (
    parsed.origin !== base.origin ||
    !value.startsWith("/") ||
    value.startsWith("//") ||
    value.includes("\\")
  ) {
    throw new AdapterError(400, "invalid_return_to");
  }
  return `${parsed.pathname}${parsed.search}`;
}

function toSessionView(session: AuthSession): SessionView {
  return {
    authenticated: true,
    user: {
      user_id: session.user_id,
      github_user_id: session.github_user_id,
      github_login: session.github_login,
      github_avatar_url: session.github_avatar_url,
    },
    workspace: {
      workspace_id: session.workspace_id,
      slug: session.workspace_slug,
      kind: "personal",
    },
  };
}

function githubCallbackUrl(url: URL): string {
  return `${url.origin}/api/v1/auth/github/callback`;
}

function cookieValue(request: Request, name: string): string | null {
  const header = request.headers.get("cookie");
  if (!header) {
    return null;
  }

  for (const part of header.split(";")) {
    const [candidate, ...rest] = part.trim().split("=");
    if (candidate === name) {
      return rest.join("=") || null;
    }
  }
  return null;
}

function cookie(
  name: string,
  value: string,
  maxAge: number,
  url: URL,
  path = "/",
): string {
  return [
    `${name}=${value}`,
    `Max-Age=${maxAge}`,
    `Path=${path}`,
    "HttpOnly",
    "SameSite=Lax",
    ...(url.protocol === "https:" ? ["Secure"] : []),
  ].join("; ");
}

function clearCookie(
  name: string,
  url: URL,
  path: string,
): string {
  return [
    `${name}=`,
    "Max-Age=0",
    `Path=${path}`,
    "HttpOnly",
    "SameSite=Lax",
    ...(url.protocol === "https:" ? ["Secure"] : []),
  ].join("; ");
}

function randomOpaque(prefix: string): string {
  const bytes = crypto.getRandomValues(new Uint8Array(32));
  let binary = "";
  for (const byte of bytes) {
    binary += String.fromCharCode(byte);
  }
  const encoded = btoa(binary)
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replace(/=+$/, "");
  return prefix + encoded;
}
