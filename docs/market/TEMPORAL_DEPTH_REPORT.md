# Hooktry MCIF temporal-depth report

Checked: 2026-10-01  
DWC: MCIF/Hooktry.11 TEMPORAL-DEPTH

## Question

The previous research pass established that CI wiring, bounded waits, reproducible environments, suites, and structured request evidence are competitive capabilities.

This pass asks the narrower question:

> Do primary competitors already expose temporal and causal assertion semantics deep enough to invalidate Hooktry's remaining differentiation hypothesis?

The evaluated dimensions are:

- structured interaction contracts
- reusable scenario runs
- cardinality assertions
- ordering assertions
- observation/settle windows
- correlation/idempotency context

## webhooks.cc

Current official evidence supports:

- **contracts: partial** - `assertRequest` declares structured expectations over one captured request and returns structured diffs
- **scenario-runs: present** - testing helpers and `test_webhook_flow` package setup, send/capture, verification/assertion, replay, and cleanup
- **cardinality: partial** - `captureDuring({ count, timeout })` waits for an expected request count and `count_requests` counts matching requests
- **ordering: partial** - multi-request capture is returned sorted by timestamp and examples assert positional requests
- **observation-window: partial** - explicit bounded timeout horizons exist
- **correlation-context: unknown** - current official evidence does not establish normalized correlation/causation/message/idempotency identifiers as first-class matching context

Important limit:

`count = N` is not equivalent to ranged `at_least / at_most` semantics plus a quiet period proving that no later matching event violates the expectation.

Likewise timestamp sorting is not equivalent to a durable observed-sequence assertion primitive.

## Hooklistener

Current official evidence supports:

- **contracts: partial** - replay cases persist expected response status and JSON-subset assertions
- **scenario-runs: present** - saved cases, suites, target execution, assertions, bounded waiting, and durable aggregate run reports
- **observation-window: partial** - suite execution has an explicit timeout horizon
- **cardinality: unknown** - aggregate case/assertion counts are not the same as cardinality assertions over matching incoming events
- **ordering: unknown** - no current official evidence establishes ordering assertions between matching integration events
- **correlation-context: unknown** - datastore variables and request metadata exist, but no current official evidence establishes normalized correlation/idempotency matching semantics

Hooklistener is therefore closer to a reusable regression-suite model than to Hooktry's temporal multi-event evidence model.

## Webhook Relay

Current official evidence supports:

- **scenario-runs: partial** - agentic testing provides a repeatable `send_webhook -> wait_for_webhook_log -> inspect evidence` flow through the real routing path
- **observation-window: partial** - `wait_for_webhook_log` has an explicit timeout and returns when one delivery settles

Its delivery documentation also discusses webhook IDs, deduplication, and idempotent receivers, but that is delivery guidance rather than a normalized assertion/matching context in the testing product.

## What this pass changes

The moat is narrower than before.

The following are **not** defensible differentiators by themselves:

- reusable scenarios/suites
- structured single-request assertions
- bounded timeouts
- expected event counts
- timestamp-sorted multi-event capture
- durable test run reports
- agentic send/wait/inspect loops

## Remaining Hooktry hypothesis

The currently less-common combination is:

```text
ranged cardinality
+ durable observed ordering
+ hard horizon + quiet/settle window
+ normalized correlation / causation / request / message / trace / idempotency context
+ persisted assertion evidence
+ behavior-mismatch vs infrastructure/tool outcome semantics
```

This can be summarized as:

> Hooktry should not differentiate by "webhook tests in CI." It should differentiate, if demand validates it, by explaining and proving asynchronous multi-event behavior under time, order, duplication, and causal context.

## Critical caveat

This pass establishes **competitor capability evidence**, not customer demand.

A technically distinctive temporal model can still be a poor product investment if users do not need it often enough.

The next evidence burden is therefore split:

1. continue competitor research only where it can invalidate the remaining temporal/causal claim
2. collect first-party Hooktry usage evidence for duplicates, ordering, settle windows, and correlation/idempotency failures

## Projection effect

This pass intentionally adds six temporal-verification rows to the primary decision matrix.

That changes the matrix denominator:

- previous external cells: 230
- current external cells: 290
- previous evidenced cells: 74
- current evidenced cells: 84
- current unknown cells: 206

The increase in unknown cells is **projection scope expansion**, not lost evidence.

Future research should not attempt to fill all 206 cells. Only decision-relevant uncertainty should be reduced.
