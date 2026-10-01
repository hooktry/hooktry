import { AdapterError, json } from "./core";
import type { Env, ScenarioUsageEvent, ScenarioUsageFeatures } from "./types";

const MAX_USAGE_EVENT_BYTES = 4096;
const MAX_COUNT = 1000;
const MAX_OBSERVATION_MS = 24 * 60 * 60 * 1000;

export async function ingestUsageEvent(
  request: Request,
  env: Env,
): Promise<Response> {
  if (!env.USAGE_INGEST_TOKEN) {
    throw new AdapterError(503, "usage_ingest_unconfigured");
  }

  if (request.headers.get("authorization") !== `Bearer ${env.USAGE_INGEST_TOKEN}`) {
    throw new AdapterError(401, "unauthorized");
  }

  const contentLength = request.headers.get("content-length");
  if (
    contentLength &&
    Number.isFinite(Number(contentLength)) &&
    Number(contentLength) > MAX_USAGE_EVENT_BYTES
  ) {
    throw new AdapterError(413, "usage_event_too_large");
  }

  const raw = await request.text();
  if (new TextEncoder().encode(raw).byteLength > MAX_USAGE_EVENT_BYTES) {
    throw new AdapterError(413, "usage_event_too_large");
  }

  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    throw new AdapterError(400, "invalid_usage_event");
  }

  const event = validateUsageEvent(value);
  const result = await env.DB.prepare(
    `INSERT OR IGNORE INTO usage_events (
       event_id, received_at, occurred_at_ms, event_type,
       passed, command_success, outcome_passed, check_count, observation_elapsed_ms,
       contract_count, exact_cardinality, ranged_cardinality, ordering_enabled,
       observation_horizon, settle_window, context_match, idempotency_context, duplicate_guard
     ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
  )
    .bind(
      event.event_id,
      Math.floor(Date.now() / 1000),
      event.occurred_at_unix_ms,
      event.event,
      bool(event.passed),
      bool(event.command_success),
      bool(event.outcome_passed),
      event.check_count,
      event.observation_elapsed_ms,
      event.features.contract_count,
      bool(event.features.exact_cardinality),
      bool(event.features.ranged_cardinality),
      bool(event.features.ordering),
      bool(event.features.observation_horizon),
      bool(event.features.settle_window),
      bool(event.features.context_match),
      bool(event.features.idempotency_context),
      bool(event.features.duplicate_guard),
    )
    .run();

  return json(
    {
      accepted: true,
      stored: result.meta.changes === 1,
    },
    202,
  );
}

function validateUsageEvent(value: unknown): ScenarioUsageEvent {
  const event = object(value);
  exactKeys(event, [
    "schema_version",
    "event_id",
    "event",
    "occurred_at_unix_ms",
    "passed",
    "command_success",
    "outcome_passed",
    "check_count",
    "observation_elapsed_ms",
    "features",
  ]);

  if (event.schema_version !== 1) invalid();
  if (
    typeof event.event_id !== "string" ||
    !/^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(
      event.event_id,
    )
  ) {
    invalid();
  }
  if (event.event !== "scenario_run_completed") invalid();

  const occurredAt = integer(event.occurred_at_unix_ms, 0, Number.MAX_SAFE_INTEGER);
  const checkCount = integer(event.check_count, 0, MAX_COUNT);
  const observationElapsed = integer(
    event.observation_elapsed_ms,
    0,
    MAX_OBSERVATION_MS,
  );

  const features = validateFeatures(event.features);

  return {
    schema_version: 1,
    event_id: event.event_id,
    event: "scenario_run_completed",
    occurred_at_unix_ms: occurredAt,
    passed: boolean(event.passed),
    command_success: boolean(event.command_success),
    outcome_passed: boolean(event.outcome_passed),
    check_count: checkCount,
    observation_elapsed_ms: observationElapsed,
    features,
  };
}

function validateFeatures(value: unknown): ScenarioUsageFeatures {
  const features = object(value);
  exactKeys(features, [
    "contract_count",
    "exact_cardinality",
    "ranged_cardinality",
    "ordering",
    "observation_horizon",
    "settle_window",
    "context_match",
    "idempotency_context",
    "duplicate_guard",
  ]);

  return {
    contract_count: integer(features.contract_count, 1, MAX_COUNT),
    exact_cardinality: boolean(features.exact_cardinality),
    ranged_cardinality: boolean(features.ranged_cardinality),
    ordering: boolean(features.ordering),
    observation_horizon: boolean(features.observation_horizon),
    settle_window: boolean(features.settle_window),
    context_match: boolean(features.context_match),
    idempotency_context: boolean(features.idempotency_context),
    duplicate_guard: boolean(features.duplicate_guard),
  };
}

function object(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) invalid();
  return value as Record<string, unknown>;
}

function exactKeys(
  value: Record<string, unknown>,
  expected: string[],
): void {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (
    actual.length !== wanted.length ||
    actual.some((key, index) => key !== wanted[index])
  ) {
    invalid();
  }
}

function integer(value: unknown, min: number, max: number): number {
  if (
    typeof value !== "number" ||
    !Number.isSafeInteger(value) ||
    value < min ||
    value > max
  ) {
    invalid();
  }
  return value;
}

function boolean(value: unknown): boolean {
  if (typeof value !== "boolean") invalid();
  return value;
}

function bool(value: boolean): number {
  return value ? 1 : 0;
}

function invalid(): never {
  throw new AdapterError(400, "invalid_usage_event");
}
