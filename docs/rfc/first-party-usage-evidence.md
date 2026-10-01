# First-party usage evidence

Status: implemented thin proof / internal opt-in collector

## Purpose

FIRST-PARTY-PROOF measures whether Ortyo's existing temporal verification primitives are actually exercised.

It is not general product analytics and must not become a second evidence store for webhook content.

The separation is:

```text
Interaction evidence
  = what crossed the integration boundary
  = may contain request/response data

Usage evidence
  = which Ortyo verification shape ran and whether it passed
  = must not contain boundary payloads or product identifiers
```

## Event boundary

The canonical event is `ScenarioUsageEvent`.

It is derived from:

- `CreateScenario` feature shape
- final `ScenarioRunReport`

It is not derived by copying the manifest, command, interactions, contracts, or exposure metadata.

This makes sensitive fields impossible to serialize accidentally through the typed event structure.

## Collection policy

No sink is configured by default.

Supported sinks:

1. explicit local JSONL through `ORTYO_USAGE_LOG`
2. explicit HTTP POST through `ORTYO_USAGE_ENDPOINT`
3. optional bearer auth for the HTTP sink through `ORTYO_USAGE_TOKEN`

Delivery is best-effort and never changes Scenario outcome or process exit semantics.

## Hosted ingestion

Cloudflare accepts usage events only when `USAGE_INGEST_TOKEN` is configured and the request presents the exact bearer token.

The endpoint:

- caps event size
- accepts only schema version 1
- accepts only `scenario_run_completed`
- requires an opaque valid UUID telemetry event ID
- rejects unknown top-level or feature keys
- inserts idempotently by event ID
- stores no raw request body after validation
- stores no user/workspace/scenario/run/exposure/interaction identity

The HTTP request body naturally exists in Worker memory while being validated; it is not persisted as a blob or copied to the webhook evidence stores.

## Non-goals

Do not add in this slice:

- automatic telemetry enabled by default
- unique-user tracking
- installation fingerprints
- workspace/user IDs
- scenario or contract names
- commands
- request paths
- headers or payloads
- arbitrary metadata maps
- OpenTelemetry spans for usage analytics
- automatic roadmap decisions based on event count

## Future identity

If product decisions later require retention or per-developer repeat-use measurement, design a separate privacy-reviewed subject model.

Do not repurpose Scenario/run IDs or GitHub identity as a shortcut.
