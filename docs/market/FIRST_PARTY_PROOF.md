# Ortyo MCIF first-party proof loop

Checked: 2026-10-01  
DWC: MCIF/ORTYO.13 FIRST-PARTY-PROOF

## Goal

Desk research has sufficiently established that duplicate processing, out-of-order delivery, and idempotency identity failures are real webhook problems.

The remaining uncertainty is product-specific:

> Do developers actually use Ortyo's temporal/causal primitives to catch those failures before production?

This slice creates the evidence loop without pretending that instrumentation itself is demand.

## Thin proof recipes

Two repository-owned portable Scenario manifests are now the canonical probes:

### Duplicate / idempotency

`examples/scenarios/duplicate-idempotency.json`

It combines:

- one logical operation
- normalized correlation + idempotency context
- effective `exactly 1` cardinality
- bounded observation horizon
- quiet/settle window

The useful outcome is not "the feature exists." The useful outcome is a run where an extra matching interaction causes a behavioral failure before production.

### Out-of-order

`examples/scenarios/out-of-order.json`

It combines:

- two individually valid interactions
- declared durable observed order
- bounded observation horizon
- quiet/settle window

The useful outcome is a run where both request contracts pass but the Scenario fails because evidence arrived reversed.

## Usage event

A completed `ortyo scenario run` can emit one privacy-minimized event when an explicit sink is configured.

The event contains:

- telemetry event ID
- event timestamp
- overall PASS/FAIL
- child command success boolean
- ScenarioOutcome success boolean
- check count
- contract count
- whether the scenario uses:
  - exact cardinality
  - ranged cardinality
  - declared ordering
  - observation horizon
  - settle window
  - context matching
  - idempotency context
  - a duplicate guard

It does **not** contain:

- Scenario, run, Exposure, Interaction, Contract, workspace, or user IDs
- scenario or contract names
- ports or URLs
- child command or arguments
- paths/operations
- headers
- request or response payloads
- correlation/idempotency values
- observation timing

Unknown extra fields are rejected by the hosted collector.

## Sinks

Telemetry is disabled by default.

### Local dogfood

```sh
ORTYO_USAGE_LOG=.ortyo/usage.jsonl \
  ortyo scenario run examples/scenarios/duplicate-idempotency.json -- ./your-test-command
```

The local JSONL path is explicit and append-only.

### Internal hosted collector

The Cloudflare worker exposes:

```text
POST /api/v1/usage-events
Authorization: Bearer <USAGE_INGEST_TOKEN>
```

The endpoint is disabled when `USAGE_INGEST_TOKEN` is absent.

A CLI can opt into the sink with:

```sh
ORTYO_USAGE_ENDPOINT=https://<host>/api/v1/usage-events \
ORTYO_USAGE_TOKEN=<same ingest token> \
  ortyo scenario run ...
```

Collector failure never changes Scenario PASS/FAIL.

The D1 table stores only the allowlisted aggregate shape.

## What v1 can measure

- total completed Scenario runs
- runs using duplicate/idempotency guards
- runs using declared ordering
- runs using settle windows
- PASS/FAIL rate for those shapes
- whether a behavioral failure is actually being surfaced by a temporal proof

Useful aggregate queries:

```sql
SELECT
  COUNT(*) AS runs,
  SUM(duplicate_guard) AS duplicate_guard_runs,
  SUM(ordering_enabled) AS ordering_runs,
  SUM(settle_window) AS settle_runs,
  SUM(CASE WHEN passed = 0 AND command_success = 1 THEN 1 ELSE 0 END)
    AS behavior_failures
FROM usage_events;
```

```sql
SELECT
  duplicate_guard,
  ordering_enabled,
  passed,
  COUNT(*) AS runs
FROM usage_events
GROUP BY duplicate_guard, ordering_enabled, passed
ORDER BY runs DESC;
```

## What v1 deliberately cannot measure

The central event has no user/workspace/install identifier.

Therefore v1 cannot reliably calculate:

- unique developers
- per-user retention
- whether the same developer reran a scenario
- whether one developer modified a scenario between runs

That is intentional.

Adding pseudonymous identity requires a separate privacy/product decision and is not smuggled into this validation slice.

Local dogfood logs can still show repeated runs within a controlled project/environment.

## Decision gates

Event volume alone must not promote the temporal DSL.

Keep `cardinality-ordering` at:

```yaml
horizon: validate
decision: differentiation
next_evidence: first_party_usage
```

until first-party evidence shows value, for example:

- a duplicate/idempotency proof catches an unintended extra side effect before production
- an ordering proof catches a real state-transition bug before production
- an independent project repeatedly uses one of the proof shapes
- a user asks for richer cardinality/order/settle/context syntax because the thin primitive is insufficient

Only then should Ortyo deepen the DSL.

## Stop condition

If dogfood/public usage shows that developers mostly use capture/replay and do not run the temporal proof shapes, keep the primitives available but stop investing in richer syntax.

The purpose of this loop is to make that outcome visible, not to force validation of the moat.
