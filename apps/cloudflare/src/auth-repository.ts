import { AdapterError, uuidV7 } from "./core";
import type {
  AuthSession,
  AuthUser,
  AuthWorkspace,
  Env,
  GitHubIdentity,
} from "./types";

export async function createOAuthState(
  env: Env,
  stateDigest: string,
  returnTo: string,
  now: number,
  expiresAt: number,
): Promise<void> {
  const result = await env.DB.prepare(
    `INSERT INTO auth_oauth_states (state_digest, return_to, created_at, expires_at)
     VALUES (?, ?, ?, ?)`,
  )
    .bind(stateDigest, returnTo, now, expiresAt)
    .run();

  if (result.meta.changes !== 1) {
    throw new AdapterError(500, "storage_error");
  }
}

export async function consumeOAuthState(
  env: Env,
  stateDigest: string,
  now: number,
): Promise<string> {
  const row = await env.DB.prepare(
    `SELECT return_to, expires_at
     FROM auth_oauth_states
     WHERE state_digest = ?`,
  )
    .bind(stateDigest)
    .first<{ return_to: string; expires_at: number }>();

  if (!row || now >= row.expires_at) {
    if (row) {
      await env.DB.prepare(
        "DELETE FROM auth_oauth_states WHERE state_digest = ?",
      )
        .bind(stateDigest)
        .run();
    }
    throw new AdapterError(400, "oauth_state_invalid");
  }

  const deleted = await env.DB.prepare(
    "DELETE FROM auth_oauth_states WHERE state_digest = ?",
  )
    .bind(stateDigest)
    .run();

  if (deleted.meta.changes !== 1) {
    throw new AdapterError(400, "oauth_state_invalid");
  }

  return row.return_to;
}

export async function upsertGitHubIdentity(
  env: Env,
  identity: GitHubIdentity,
  now: number,
): Promise<{ user: AuthUser; workspace: AuthWorkspace }> {
  const candidateUserId = uuidV7();

  await env.DB.prepare(
    `INSERT INTO auth_users (
       user_id, github_user_id, github_login, github_avatar_url, created_at, updated_at
     )
     VALUES (?, ?, ?, ?, ?, ?)
     ON CONFLICT(github_user_id) DO UPDATE SET
       github_login = excluded.github_login,
       github_avatar_url = excluded.github_avatar_url,
       updated_at = excluded.updated_at`,
  )
    .bind(
      candidateUserId,
      identity.github_user_id,
      identity.github_login,
      identity.github_avatar_url,
      now,
      now,
    )
    .run();

  const user = await env.DB.prepare(
    `SELECT user_id, github_user_id, github_login, github_avatar_url, created_at, updated_at
     FROM auth_users
     WHERE github_user_id = ?`,
  )
    .bind(identity.github_user_id)
    .first<AuthUser>();

  if (!user) {
    throw new AdapterError(500, "storage_error");
  }

  const workspaceId = uuidV7();
  const slug = `personal-${user.user_id.replaceAll("-", "")}`;
  await env.DB.prepare(
    `INSERT OR IGNORE INTO auth_workspaces (
       workspace_id, slug, kind, owner_user_id, created_at
     )
     VALUES (?, ?, 'personal', ?, ?)`,
  )
    .bind(workspaceId, slug, user.user_id, now)
    .run();

  const workspace = await env.DB.prepare(
    `SELECT workspace_id, slug, kind, owner_user_id, created_at
     FROM auth_workspaces
     WHERE owner_user_id = ?
       AND kind = 'personal'`,
  )
    .bind(user.user_id)
    .first<AuthWorkspace>();

  if (!workspace) {
    throw new AdapterError(500, "storage_error");
  }

  return { user, workspace };
}

export async function createSession(
  env: Env,
  sessionDigest: string,
  userId: string,
  workspaceId: string,
  now: number,
  expiresAt: number,
): Promise<void> {
  const result = await env.DB.prepare(
    `INSERT INTO auth_sessions (
       session_digest, user_id, workspace_id, created_at, expires_at
     )
     VALUES (?, ?, ?, ?, ?)`,
  )
    .bind(sessionDigest, userId, workspaceId, now, expiresAt)
    .run();

  if (result.meta.changes !== 1) {
    throw new AdapterError(500, "storage_error");
  }
}

export async function findSession(
  env: Env,
  sessionDigest: string,
  now: number,
): Promise<AuthSession | null> {
  const session = await env.DB.prepare(
    `SELECT
       s.user_id,
       s.workspace_id,
       s.expires_at,
       u.github_user_id,
       u.github_login,
       u.github_avatar_url,
       w.slug AS workspace_slug
     FROM auth_sessions s
     JOIN auth_users u ON u.user_id = s.user_id
     JOIN auth_workspaces w ON w.workspace_id = s.workspace_id
     WHERE s.session_digest = ?`,
  )
    .bind(sessionDigest)
    .first<AuthSession>();

  if (!session) {
    return null;
  }

  if (now >= session.expires_at) {
    await env.DB.prepare(
      "DELETE FROM auth_sessions WHERE session_digest = ?",
    )
      .bind(sessionDigest)
      .run();
    return null;
  }

  return session;
}

export async function deleteSession(
  env: Env,
  sessionDigest: string,
): Promise<void> {
  await env.DB.prepare(
    "DELETE FROM auth_sessions WHERE session_digest = ?",
  )
    .bind(sessionDigest)
    .run();
}

export async function cleanupExpiredAuth(
  env: Env,
  now: number,
): Promise<void> {
  await env.DB.batch([
    env.DB.prepare(
      "DELETE FROM auth_oauth_states WHERE expires_at <= ?",
    ).bind(now),
    env.DB.prepare(
      "DELETE FROM auth_sessions WHERE expires_at <= ?",
    ).bind(now),
  ]);
}
