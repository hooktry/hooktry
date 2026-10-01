export interface Env {
  ASSETS: Fetcher;
  DB: D1Database;
  PAYLOADS: R2Bucket;
  EXPOSURES: DurableObjectNamespace;
  CLAIM_INTERNAL_TOKEN?: string;
}

export interface AnonymousExposureRow {
  exposure_id: string;
  principal_digest: string;
  ingress_capability_digest: string;
  view_capability_digest: string;
  claim_capability_digest: string | null;
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

export interface AnonymousProvision extends AnonymousExposureSummary {
  hook_url: string;
  view_url: string;
  view_websocket_url: string;
  claim_url: string;
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
