CREATE TABLE anonymous_exposures (
  exposure_id TEXT PRIMARY KEY,
  principal_digest TEXT NOT NULL,
  ingress_digest TEXT NOT NULL UNIQUE,
  viewer_digest TEXT NOT NULL UNIQUE,
  claim_digest TEXT UNIQUE,
  workspace_id TEXT,
  created_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL,
  claimed_at INTEGER,
  request_count INTEGER NOT NULL DEFAULT 0,
  retained_bytes INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX anonymous_exposures_principal
  ON anonymous_exposures(principal_digest, workspace_id, expires_at);

CREATE INDEX anonymous_exposures_ingress
  ON anonymous_exposures(ingress_digest);

CREATE INDEX anonymous_exposures_viewer
  ON anonymous_exposures(viewer_digest);

CREATE INDEX anonymous_exposures_claim
  ON anonymous_exposures(claim_digest);

CREATE TABLE anonymous_interactions (
  interaction_id TEXT PRIMARY KEY,
  exposure_id TEXT NOT NULL,
  sequence INTEGER NOT NULL,
  received_at_ms INTEGER NOT NULL,
  method TEXT NOT NULL,
  path TEXT NOT NULL,
  query TEXT,
  headers_json TEXT NOT NULL,
  body_key TEXT NOT NULL,
  body_bytes INTEGER NOT NULL,
  content_type TEXT,
  UNIQUE(exposure_id, sequence)
);

CREATE INDEX anonymous_interactions_exposure
  ON anonymous_interactions(exposure_id, sequence);
