CREATE TABLE auth_users (
  user_id TEXT PRIMARY KEY,
  github_user_id TEXT NOT NULL UNIQUE,
  github_login TEXT NOT NULL,
  github_avatar_url TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE TABLE auth_workspaces (
  workspace_id TEXT PRIMARY KEY,
  slug TEXT NOT NULL UNIQUE,
  kind TEXT NOT NULL CHECK (kind IN ('personal')),
  owner_user_id TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE UNIQUE INDEX auth_personal_workspace_owner
  ON auth_workspaces(owner_user_id)
  WHERE kind = 'personal';

CREATE TABLE auth_sessions (
  session_digest TEXT PRIMARY KEY,
  user_id TEXT NOT NULL,
  workspace_id TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
);

CREATE INDEX auth_sessions_expires
  ON auth_sessions(expires_at);

CREATE INDEX auth_sessions_user
  ON auth_sessions(user_id, expires_at);

CREATE TABLE auth_oauth_states (
  state_digest TEXT PRIMARY KEY,
  return_to TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
);

CREATE INDEX auth_oauth_states_expires
  ON auth_oauth_states(expires_at);
