import type {
  AnonymousExposureRow,
  AnonymousExposureSummary,
  AnonymousInteraction,
  AnonymousInteractionRow,
} from "./types";

export const ANONYMOUS_TTL_SECONDS = 5 * 24 * 60 * 60;
export const ANONYMOUS_REQUEST_LIMIT = 100;
export const ANONYMOUS_ACTIVE_LIMIT = 3;
export const ANONYMOUS_MAX_BODY_BYTES = 5 * 1024 * 1024;
export const ANONYMOUS_MAX_RETAINED_BYTES = 50 * 1024 * 1024;

export class AdapterError extends Error {
  constructor(
    public readonly status: number,
    public readonly code: string,
  ) {
    super(code);
  }
}

export function json(value: unknown, status = 200): Response {
  return Response.json(value, { status });
}

export function errorResponse(error: unknown): Response {
  if (error instanceof AdapterError) {
    return json({ error: { code: error.code } }, error.status);
  }
  console.error(error);
  return json({ error: { code: "internal_error" } }, 500);
}

export function summaryFromRow(
  row: AnonymousExposureRow,
  nowSeconds: number,
): AnonymousExposureSummary {
  const claimed = row.workspace_id !== null;
  if (!claimed && nowSeconds >= row.expires_at) {
    throw new AdapterError(410, "expired");
  }
  return {
    exposure_id: row.exposure_id,
    ...(row.workspace_id ? { workspace_id: row.workspace_id } : {}),
    created_at_unix_seconds: row.created_at,
    ...(claimed ? {} : { expires_at_unix_seconds: row.expires_at }),
    request_count: row.request_count,
    retained_bytes: row.retained_bytes,
    request_limit: ANONYMOUS_REQUEST_LIMIT,
    max_body_bytes: ANONYMOUS_MAX_BODY_BYTES,
    max_retained_bytes: ANONYMOUS_MAX_RETAINED_BYTES,
    claimed,
  };
}

export async function interactionFromRow(
  row: AnonymousInteractionRow,
  payloads: R2Bucket,
): Promise<AnonymousInteraction> {
  const object = await payloads.get(row.body_key);
  if (!object) {
    throw new AdapterError(500, "payload_missing");
  }
  const body = new Uint8Array(await object.arrayBuffer());
  const decoded = decodeBody(body);
  return {
    interaction_id: row.interaction_id,
    exposure_id: row.exposure_id,
    sequence: row.sequence,
    received_at_unix_ms: row.received_at_ms,
    method: row.method,
    path: row.path,
    ...(row.query ? { query: row.query } : {}),
    headers: JSON.parse(row.headers_json) as Array<[string, string]>,
    body_encoding: decoded.encoding,
    body: decoded.body,
    body_bytes: row.body_bytes,
  };
}

export function decodeBody(
  body: Uint8Array,
): { encoding: "utf8" | "hex"; body: string } {
  try {
    return {
      encoding: "utf8",
      body: new TextDecoder("utf-8", { fatal: true, ignoreBOM: false }).decode(body),
    };
  } catch {
    return {
      encoding: "hex",
      body: Array.from(body, (byte) => byte.toString(16).padStart(2, "0")).join(""),
    };
  }
}

export async function sha256Hex(value: string): Promise<string> {
  const bytes = new TextEncoder().encode(value);
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  return Array.from(digest, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

export function randomPrincipal(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(32));
  return `hooktry_ap_${hex(bytes)}`;
}

export function randomCapability(prefix: "hk_" | "vw_" | "cl_"): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return prefix + hex(bytes);
}

export function uuidV7(nowMs = Date.now()): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  const timestamp = BigInt(nowMs);
  bytes[0] = Number((timestamp >> 40n) & 0xffn);
  bytes[1] = Number((timestamp >> 32n) & 0xffn);
  bytes[2] = Number((timestamp >> 24n) & 0xffn);
  bytes[3] = Number((timestamp >> 16n) & 0xffn);
  bytes[4] = Number((timestamp >> 8n) & 0xffn);
  bytes[5] = Number(timestamp & 0xffn);
  bytes[6] = (bytes[6] & 0x0f) | 0x70;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  const value = hex(bytes);
  return [
    value.slice(0, 8),
    value.slice(8, 12),
    value.slice(12, 16),
    value.slice(16, 20),
    value.slice(20),
  ].join("-");
}

export function validAnonymousPrincipal(value: string): boolean {
  return /^hooktry_ap_[0-9a-f]{64}$/.test(value);
}

export function validWorkspaceId(value: string): boolean {
  return /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
    value,
  );
}

export function anonymousPrincipal(request: Request): string | null {
  const header = request.headers.get("x-hooktry-anonymous-principal");
  if (header && validAnonymousPrincipal(header)) {
    return header;
  }
  const cookie = request.headers.get("cookie");
  if (!cookie) {
    return null;
  }
  for (const entry of cookie.split(";")) {
    const [name, ...parts] = entry.trim().split("=");
    const value = parts.join("=");
    if (name === "hooktry_anon" && validAnonymousPrincipal(value)) {
      return value;
    }
  }
  return null;
}

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

