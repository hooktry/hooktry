# Ortyo MCIF trend report

Checked: 2026-10-01

## Current comparison

`2026-10-01-research-2 -> 2026-10-01-research-3`

This pass expands the decision projection rather than claiming that competitors changed.

## Result

Six existing canonical capabilities were added to the product matrix:

- contracts
- scenario-runs
- cardinality
- ordering
- observation-window
- correlation-context

The matrix therefore expands from 23 to 29 rows:

- external cells: 230 -> 290
- previously projected evidenced cells retained: 74
- evidence-backed states on newly projected rows: 10
- current evidenced external cells: 84
- current unknown external cells: 206
- atomic observations: 84 -> 94

The trend analyzer should classify the new rows as **projection_scope_change**, not research regression and not market motion.

## Temporal-depth evidence added

webhooks.cc:

- contracts -> partial
- scenario-runs -> present
- cardinality -> partial
- ordering -> partial
- observation-window -> partial

Hooklistener:

- contracts -> partial
- scenario-runs -> present
- observation-window -> partial

Webhook Relay:

- scenario-runs -> partial
- observation-window -> partial

Correlation/idempotency context remains unknown for these competitors under the normalized Ortyo capability definition.

## Decision impact

No capability disposition changed.

No Ortyo implementation status changed.

No explicit market-motion signal was added.

The differentiation hypothesis became narrower, but the roadmap did not move.

## Remaining hypothesis

Competitor evidence now covers reusable suites/scenarios, single-request contracts, expected counts, timestamp-sorted multi-event capture, and bounded waits.

The remaining Ortyo hypothesis is:

```text
ranged cardinality + proof of no later extra matches
+ durable observed-order assertions
+ hard horizon + quiet/settle window
+ normalized correlation/causation/request/message/trace/idempotency context
+ persisted assertion evidence
+ behavior mismatch vs infrastructure/tool outcome semantics
```

## Commands

Latest projection diff:

```bash
cargo run --locked --bin market-trend -- \
  --from docs/market/snapshots/2026-10-01-research-2.yaml \
  --to docs/market/snapshots/2026-10-01-research-3.yaml
```

Cumulative diff from the original baseline:

```bash
cargo run --locked --bin market-trend -- \
  --from docs/market/snapshots/2026-10-01-baseline.yaml \
  --to docs/market/snapshots/2026-10-01-research-3.yaml \
  --json
```
