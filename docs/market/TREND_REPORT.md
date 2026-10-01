# Ortyo MCIF trend report

Checked: 2026-10-01

## Current comparison

`2026-10-01-research-3 -> 2026-10-01-demand-1`

This pass changes the **demand evidence ledger**, not competitor capability state.

## Result

- scope products added/removed: 0 / 0
- matrix products added/removed: 0 / 0
- capabilities added/removed: 0 / 0
- matrix changes: 0
- atomic observations added/removed: 0 / 0
- signals added: 9
- direct-demand signals added: 6
- demand-proxy signals added: 3
- explicit market-motion signals added: 0
- Ortyo capability-status changes: 0
- Ortyo matrix changes: 0
- capability disposition changes: 0

The market matrix remains:

- external cells: 290
- evidenced: 84
- unknown: 206

## What changed

The new direct evidence validates the temporal failure **problem family**:

- duplicate deliveries causing duplicate workflow execution
- duplicate Stripe deliveries causing duplicate notifications
- out-of-order status webhooks regressing state
- ordering/eventual-consistency races producing wrong entitlement state
- unstable event identity making downstream deduplication impossible
- incorrectly scoped delivery identity suppressing a legitimate second route

Provider docs add independent demand proxies for:

- duplicate/retry delivery identity
- non-guaranteed event ordering
- variable/delayed delivery timing

## Decision impact

The `cardinality-ordering` priority remains:

```text
horizon: validate
decision: differentiation
```

Confidence rises from low to medium.

The next evidence channel changes to:

```yaml
next_evidence: first_party_usage
```

This means competitor matrix unknowns for `cardinality`, `ordering`, and `correlation-context` should no longer automatically rise into the top external-research queue merely because direct demand is strong.

The existence of the problem is sufficiently established. The open question is whether Ortyo's abstraction is useful in practice.

## Interpretation boundary

This pass does **not** prove demand for:

- ranged cardinality syntax
- durable-order predicate syntax
- quiet/settle windows
- a unified correlation/causation/idempotency DSL
- Contract/Scenario as the preferred authoring surface

It proves that the underlying duplicate/order/identity failure classes are real and recurrent.

## Commands

Demand-proof diff:

```bash
cargo run --locked --bin market-trend -- \
  --from docs/market/snapshots/2026-10-01-research-3.yaml \
  --to docs/market/snapshots/2026-10-01-demand-1.yaml
```

Research-debt queue:

```bash
cargo run --locked --bin market-research-debt -- \
  --limit 20 \
  --max-per-capability 3 \
  --json
```
