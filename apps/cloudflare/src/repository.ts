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
  const configuredActiveLimit = Number.parseInt(env.HOOKTRY_ACTIVE_LIMIT ?? "", 10);
  const activeLimit =
    Number.isFinite(configuredActiveLimit) && configuredActiveLimit > 0
      ? configuredActiveLimit
      : ANONYMOUS_ACTIVE_LIMIT;

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
      activeLimit,
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
            handoff_capability_digest, handoff_payload_ciphertext, handoff_payload_nonce,
            handoff_expires_at, handoff_consumed_at,
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
            handoff_capability_digest, handoff_payload_ciphertext, handoff_payload_nonce,
            handoff_expires_at, handoff_consumed_at,
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
            handoff_capability_digest, handoff_payload_ciphertext, handoff_payload_nonce,
            handoff_expires_at, handoff_consumed_at,
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
            handoff_capability_digest, handoff_payload_ciphertext, handoff_payload_nonce,
            handoff_expires_at, handoff_consumed_at,
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
     SET workspace_id = ?,
         claim_capability_digest = NULL,
         claimed_at = ?,
         handoff_capability_digest = NULL,
         handoff_payload_ciphertext = NULL,
         handoff_payload_nonce = NULL,
         handoff_expires_at = NULL,
         handoff_consumed_at = COALESCE(handoff_consumed_at, ?)
     WHERE exposure_id = ?
       AND claim_capability_digest = ?
       AND workspace_id IS NULL
       AND expires_at > ?`,
  )
    .bind(workspaceId, now, now, row.exposure_id, claimCapabilityDigest, now)
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

export async function installHandoff(
  env: Env,
  input: {
    exposureId: string;
    capabilityDigest: string;
    ciphertext: string;
    nonce: string;
    expiresAt: number;
    now: number;
  },
): Promise<void> {
  const result = await env.DB.prepare(
    `UPDATE anonymous_exposures
     SET handoff_capability_digest = ?,
         handoff_payload_ciphertext = ?,
         handoff_payload_nonce = ?,
         handoff_expires_at = ?,
         handoff_consumed_at = NULL
     WHERE exposure_id = ?
       AND workspace_id IS NULL
       AND expires_at > ?`,
  )
    .bind(
      input.capabilityDigest,
      input.ciphertext,
      input.nonce,
      input.expiresAt,
      input.exposureId,
      input.now,
    )
    .run();

  if (result.meta.changes !== 1) {
    throw new AdapterError(410, "invalid_handoff");
  }
}

export async function readHandoff(
  env: Env,
  capabilityDigest: string,
  now: number,
): Promise<{
  exposure_id: string;
  handoff_payload_ciphertext: string;
  handoff_payload_nonce: string;
} | null> {
  return env.DB.prepare(
    `SELECT exposure_id, handoff_payload_ciphertext, handoff_payload_nonce
     FROM anonymous_exposures
     WHERE handoff_capability_digest = ?
       AND handoff_payload_ciphertext IS NOT NULL
       AND handoff_payload_nonce IS NOT NULL
       AND handoff_consumed_at IS NULL
       AND handoff_expires_at > ?
       AND workspace_id IS NULL
       AND expires_at > ?`,
  )
    .bind(capabilityDigest, now, now)
    .first<{
      exposure_id: string;
      handoff_payload_ciphertext: string;
      handoff_payload_nonce: string;
    }>();
}

export async function consumeHandoff(
  env: Env,
  input: {
    exposureId: string;
    capabilityDigest: string;
    now: number;
  },
): Promise<void> {
  const result = await env.DB.prepare(
    `UPDATE anonymous_exposures
     SET handoff_capability_digest = NULL,
         handoff_payload_ciphertext = NULL,
         handoff_payload_nonce = NULL,
         handoff_expires_at = NULL,
         handoff_consumed_at = ?
     WHERE exposure_id = ?
       AND handoff_capability_digest = ?
       AND handoff_consumed_at IS NULL
       AND handoff_expires_at > ?
       AND workspace_id IS NULL
       AND expires_at > ?`,
  )
    .bind(
      input.now,
      input.exposureId,
      input.capabilityDigest,
      input.now,
      input.now,
    )
    .run();

  if (result.meta.changes !== 1) {
    throw new AdapterError(410, "invalid_handoff");
  }
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
