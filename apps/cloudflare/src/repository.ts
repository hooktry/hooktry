import {
  ANONYMOUS_ACTIVE_LIMIT,
  AdapterError,
  summaryFromRow,
} from "./core";
import type {
  AnonymousExposureRow,
  AnonymousExposureSummary,
  AnonymousInteractionRow,
  Env,
} from "./types";

export async function createExposure(
  env: Env,
  input: {
    exposureId: string;
    principalDigest: string;
    ingressCapabilityDigest: string;
    viewCapabilityDigest: string;
    claimCapabilityDigest: string;
    now: number;
    expiresAt: number;
  },
): Promise<AnonymousExposureSummary> {
  const result = await env.DB.prepare(
    `INSERT INTO anonymous_exposures (
       exposure_id, principal_digest, ingress_capability_digest, view_capability_digest, claim_capability_digest,
       workspace_id, created_at, expires_at, claimed_at, request_count, retained_bytes
     )
     SELECT ?, ?, ?, ?, ?, NULL, ?, ?, NULL, 0, 0
     WHERE (
       SELECT COUNT(*)
       FROM anonymous_exposures
       WHERE principal_digest = ?
         AND workspace_id IS NULL
         AND expires_at > ?
     ) < ?`,
  )
    .bind(
      input.exposureId,
      input.principalDigest,
      input.ingressCapabilityDigest,
      input.viewCapabilityDigest,
      input.claimCapabilityDigest,
      input.now,
      input.expiresAt,
      input.principalDigest,
      input.now,
      ANONYMOUS_ACTIVE_LIMIT,
    )
    .run();

  if (result.meta.changes !== 1) {
    throw new AdapterError(429, "active_limit");
  }

  const row = await getExposureById(env, input.exposureId);
  if (!row) {
    throw new AdapterError(500, "storage_error");
  }
  return summaryFromRow(row, input.now);
}

export async function getExposureById(
  env: Env,
  exposureId: string,
): Promise<AnonymousExposureRow | null> {
  return env.DB.prepare(
    `SELECT exposure_id, principal_digest, ingress_capability_digest, view_capability_digest, claim_capability_digest,
            workspace_id, created_at, expires_at, claimed_at, request_count, retained_bytes
     FROM anonymous_exposures
     WHERE exposure_id = ?`,
  )
    .bind(exposureId)
    .first<AnonymousExposureRow>();
}

export async function findExposureByIngressCapability(
  env: Env,
  digest: string,
): Promise<AnonymousExposureRow | null> {
  return env.DB.prepare(
    `SELECT exposure_id, principal_digest, ingress_capability_digest, view_capability_digest, claim_capability_digest,
            workspace_id, created_at, expires_at, claimed_at, request_count, retained_bytes
     FROM anonymous_exposures
     WHERE ingress_capability_digest = ?`,
  )
    .bind(digest)
    .first<AnonymousExposureRow>();
}

export async function findExposureByViewCapability(
  env: Env,
  digest: string,
): Promise<AnonymousExposureRow | null> {
  return env.DB.prepare(
    `SELECT exposure_id, principal_digest, ingress_capability_digest, view_capability_digest, claim_capability_digest,
            workspace_id, created_at, expires_at, claimed_at, request_count, retained_bytes
     FROM anonymous_exposures
     WHERE view_capability_digest = ?`,
  )
    .bind(digest)
    .first<AnonymousExposureRow>();
}

export async function claimExposure(
  env: Env,
  claimCapabilityDigest: string,
  workspaceId: string,
  now: number,
): Promise<AnonymousExposureSummary> {
  const row = await env.DB.prepare(
    `SELECT exposure_id, principal_digest, ingress_capability_digest, view_capability_digest, claim_capability_digest,
            workspace_id, created_at, expires_at, claimed_at, request_count, retained_bytes
     FROM anonymous_exposures
     WHERE claim_capability_digest = ?`,
  )
    .bind(claimCapabilityDigest)
    .first<AnonymousExposureRow>();

  if (!row || row.workspace_id !== null || now >= row.expires_at) {
    throw new AdapterError(410, "invalid_claim");
  }

  const result = await env.DB.prepare(
    `UPDATE anonymous_exposures
     SET workspace_id = ?, claim_capability_digest = NULL, claimed_at = ?
     WHERE exposure_id = ?
       AND claim_capability_digest = ?
       AND workspace_id IS NULL
       AND expires_at > ?`,
  )
    .bind(workspaceId, now, row.exposure_id, claimCapabilityDigest, now)
    .run();

  if (result.meta.changes !== 1) {
    throw new AdapterError(410, "invalid_claim");
  }

  const claimed = await getExposureById(env, row.exposure_id);
  if (!claimed) {
    throw new AdapterError(500, "storage_error");
  }
  return summaryFromRow(claimed, now);
}

export async function listInteractions(
  env: Env,
  exposureId: string,
): Promise<AnonymousInteractionRow[]> {
  const result = await env.DB.prepare(
    `SELECT interaction_id, exposure_id, sequence, received_at_ms, method, path, query,
            headers_json, body_key, body_bytes, content_type
     FROM anonymous_interactions
     WHERE exposure_id = ?
     ORDER BY sequence`,
  )
    .bind(exposureId)
    .all<AnonymousInteractionRow>();
  return result.results;
}

export async function expiredExposureIds(
  env: Env,
  now: number,
  limit = 100,
): Promise<string[]> {
  const result = await env.DB.prepare(
    `SELECT exposure_id
     FROM anonymous_exposures
     WHERE workspace_id IS NULL
       AND expires_at <= ?
     ORDER BY expires_at
     LIMIT ?`,
  )
    .bind(now, limit)
    .all<{ exposure_id: string }>();
  return result.results.map((row) => row.exposure_id);
}

export async function payloadKeysForExposure(
  env: Env,
  exposureId: string,
): Promise<string[]> {
  const result = await env.DB.prepare(
    `SELECT body_key
     FROM anonymous_interactions
     WHERE exposure_id = ?`,
  )
    .bind(exposureId)
    .all<{ body_key: string }>();
  return result.results.map((row) => row.body_key);
}

export async function deleteExposure(
  env: Env,
  exposureId: string,
): Promise<void> {
  await env.DB.batch([
    env.DB.prepare(
      "DELETE FROM anonymous_interactions WHERE exposure_id = ?",
    ).bind(exposureId),
    env.DB.prepare(
      `DELETE FROM anonymous_exposures
       WHERE exposure_id = ?
         AND workspace_id IS NULL`,
    ).bind(exposureId),
  ]);
}
