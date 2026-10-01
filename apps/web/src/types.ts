export interface ExposureSummary {
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

export interface HookProvision extends ExposureSummary {
  hook_url: string;
  view_url: string;
  view_websocket_url: string;
  claim_url: string;
  anonymous_principal: string;
}

export interface Interaction {
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

export type StreamFrame =
  | { type: "ready"; exposure: ExposureSummary }
  | { type: "interaction"; interaction: Interaction }
  | { type: "resync_required" };


export type AuthSession =
  | { authenticated: false }
  | {
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
    };
