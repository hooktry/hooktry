export interface Env {
  ASSETS: Fetcher;
  DB: D1Database;
  PAYLOADS: R2Bucket;
  EXPOSURES: DurableObjectNamespace;
  CLAIM_INTERNAL_TOKEN?: string;
  GITHUB_CLIENT_ID?: string;
  GITHUB_CLIENT_SECRET?: string;
  USAGE_INGEST_TOKEN?: string;
  HOOKTRY_RELEASE_SHA?: string;
  HOOKTRY_PUBLIC_ORIGIN?: string;
}

export interface AnonymousExposureRow {
  exposure_id: string;
  principal_digest: string;
  ingress_capability_digest: string;
  view_capability_digest: string;
  claim_capability_digest: string | null;
  handoff_capability_digest: string | null;
  handoff_payload_ciphertext: string | null;
  handoff_payload_nonce: string | null;
  handoff_expires_at: number | null;
  handoff_consumed_at: number | null;
  workspace_id: string | null;
  created_at: number;
  expires_at: number;
  claimed_at: number | null;
  request_count: number;
  retained_bytes: number;
}

export interface AnonymousInteractionRow {
  interaction_id: string;
  exposure_id: string;
  sequence: number;
  received_at_ms: number;
  method: string;
  path: string;
  query: string | null;
  headers_json: string;
  body_key: string;
  body_bytes: number;
  content_type: string | null;
}

export interface AnonymousExposureSummary {
  exposure_id: string;
  workspace_id?: string;
  created_at_unix_seconds: number;
  expires_at_unix_seconds?: number;
  request_count: number;
  retained_bytes: number;
  request_limit: number;
  max_body_bytes: number;
  max_retained_bytes: number;
  claimed: boolean;
}

export interface AnonymousOwnerProvision extends AnonymousExposureSummary {
  hook_url: string;
  view_url: string;
  view_websocket_url: string;
  claim_url: string;
}

export interface AnonymousProvision extends AnonymousOwnerProvision {
  handoff_url: string;
  handoff_expires_at_unix_seconds: number;
  anonymous_principal: string;
}

export interface AnonymousInteraction {
  interaction_id: string;
  exposure_id: string;
  sequence: number;
  received_at_unix_ms: number;
  method: string;
  path: string;
  query?: string;
  headers: Array<[string, string]>;
  body_encoding: "utf8" | "hex";
  body: string;
  body_bytes: number;
}


export interface GitHubIdentity {
  github_user_id: string;
  github_login: string;
  github_avatar_url: string | null;
}

export interface AuthUser {
  user_id: string;
  github_user_id: string;
  github_login: string;
  github_avatar_url: string | null;
  created_at: number;
  updated_at: number;
}

export interface AuthWorkspace {
  workspace_id: string;
  slug: string;
  kind: "personal";
  owner_user_id: string;
  created_at: number;
}

export interface AuthSession {
  user_id: string;
  workspace_id: string;
  expires_at: number;
  github_user_id: string;
  github_login: string;
  github_avatar_url: string | null;
  workspace_slug: string;
}

export interface SessionView {
  authenticated: true;
  user: {
    user_id: string;
    github_user_id: string;
    github_login: string;
    github_avatar_url: string | null;
  };
  workspace: {
    workspace_id: string;
    slug: string;
    kind: "personal";
  };
}


export interface ScenarioUsageFeatures {
  contract_count: number;
  exact_cardinality: boolean;
  ranged_cardinality: boolean;
  ordering: boolean;
  observation_horizon: boolean;
  settle_window: boolean;
  context_match: boolean;
  idempotency_context: boolean;
  duplicate_guard: boolean;
}

export interface ScenarioUsageEvent {
  schema_version: 1;
  event_id: string;
  event: "scenario_run_completed";
  occurred_at_unix_ms: number;
  passed: boolean;
  command_success: boolean;
  outcome_passed: boolean;
  check_count: number;
  features: ScenarioUsageFeatures;
}
