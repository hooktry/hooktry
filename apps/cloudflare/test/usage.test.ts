import { env } from "cloudflare:workers";
import { beforeEach, describe, expect, it } from "vitest";

import worker from "../src/index";
import type { Env, ScenarioUsageEvent } from "../src/types";

const bindings = env as unknown as Env;

beforeEach(async () => {
  await bindings.DB.prepare("DELETE FROM usage_events").run();
});

describe("FIRST-PARTY-PROOF usage ingest", () => {
  it("stores only the allowlisted scenario proof shape", async () => {
    const response = await worker.fetch(
      new Request("https://ortyo.test/api/v1/usage-events", {
        method: "POST",
        headers: {
          authorization: "Bearer test-usage-token",
          "content-type": "application/json",
        },
        body: JSON.stringify(event()),
      }),
      bindings,
    );

    expect(response.status).toBe(202);
    expect(await response.json()).toEqual({
      accepted: true,
      stored: true,
    });

    const stored = await bindings.DB.prepare(
      `SELECT event_type, passed, command_success, outcome_passed,
              contract_count, exact_cardinality, ranged_cardinality,
              ordering_enabled, observation_horizon, settle_window,
              context_match, idempotency_context, duplicate_guard
       FROM usage_events`,
    ).first<Record<string, unknown>>();

    expect(stored).toEqual({
      event_type: "scenario_run_completed",
      passed: 0,
      command_success: 1,
      outcome_passed: 0,
      contract_count: 1,
      exact_cardinality: 1,
      ranged_cardinality: 0,
      ordering_enabled: 0,
      observation_horizon: 1,
      settle_window: 1,
      context_match: 1,
      idempotency_context: 1,
      duplicate_guard: 1,
    });
  });

  it("is idempotent by telemetry event id", async () => {
    const request = () =>
      new Request("https://ortyo.test/api/v1/usage-events", {
        method: "POST",
        headers: {
          authorization: "Bearer test-usage-token",
          "content-type": "application/json",
        },
        body: JSON.stringify(event()),
      });

    const first = await worker.fetch(request(), bindings);
    const second = await worker.fetch(request(), bindings);

    expect(await first.json()).toEqual({ accepted: true, stored: true });
    expect(await second.json()).toEqual({ accepted: true, stored: false });

    const count = await bindings.DB.prepare(
      "SELECT COUNT(*) AS count FROM usage_events",
    ).first<{ count: number }>();
    expect(count?.count).toBe(1);
  });

  it("rejects unauthenticated and expanded payload-bearing schemas", async () => {
    const unauthorized = await worker.fetch(
      new Request("https://ortyo.test/api/v1/usage-events", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(event()),
      }),
      bindings,
    );
    expect(unauthorized.status).toBe(401);

    const expanded = {
      ...event(),
      body: "captured webhook payload must never enter usage evidence",
    };
    const invalid = await worker.fetch(
      new Request("https://ortyo.test/api/v1/usage-events", {
        method: "POST",
        headers: {
          authorization: "Bearer test-usage-token",
          "content-type": "application/json",
        },
        body: JSON.stringify(expanded),
      }),
      bindings,
    );

    expect(invalid.status).toBe(400);
    expect(await invalid.json()).toEqual({
      error: { code: "invalid_usage_event" },
    });
    const count = await bindings.DB.prepare(
      "SELECT COUNT(*) AS count FROM usage_events",
    ).first<{ count: number }>();
    expect(count?.count).toBe(0);
  });
});

function event(): ScenarioUsageEvent {
  return {
    schema_version: 1,
    event_id: "018f6f0e-7b7b-7abc-8def-0123456789ab",
    event: "scenario_run_completed",
    occurred_at_unix_ms: 1_790_891_200_000,
    passed: false,
    command_success: true,
    outcome_passed: false,
    check_count: 1,
    observation_elapsed_ms: 712,
    features: {
      contract_count: 1,
      exact_cardinality: true,
      ranged_cardinality: false,
      ordering: false,
      observation_horizon: true,
      settle_window: true,
      context_match: true,
      idempotency_context: true,
      duplicate_guard: true,
    },
  };
}
