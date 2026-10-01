# Ortyo MCIF trend report

Checked: 2026-10-01

## Current comparison

`2026-10-01-baseline -> 2026-10-01-research-1`

This is the first real snapshot comparison.

The two snapshots are on the same date because the second snapshot captures a later research pass, not a claim that the market changed during the day.

## Result

- scope products added/removed: 0 / 0
- matrix products added/removed: 0 / 0
- capabilities added/removed: 0 / 0
- matrix research resolutions: 11
- observed external state changes: 0
- atomic observations added: 11
- signals added: 0
- explicit market-motion signals added: 0
- Ortyo capability-status changes: 0
- Ortyo matrix changes: 0
- disposition changes: 0

The 11 matrix transitions are all `unknown -> present|partial`, so they are **research resolution**, not market motion.

## Decision impact

The research changed confidence and competitive depth knowledge, but did not change the current dispositions:

- search/filter remains a current `must`
- configurable sender response remains a current `must`
- failure simulation remains current `should`
- provider-aware templates/signature verification remain next `should`
- deterministic CI remains a differentiation thesis to validate by depth, not mere presence

## Interpretation boundary

A matrix transition is not automatically a market change.

`unknown -> present|partial|absent` is **research resolution** because the earlier snapshot did not know the state.

`present|partial|absent -> unknown` is **research regression** because confidence/evidence was lost.

A transition between two evidenced non-`unknown` states is an **observed state change**. It may support a market-change claim, but the claim should still be tied to dated evidence.

Explicit supply-side change is represented by a newly added signal with:

```yaml
class: market_motion
```

Ortyo changes are reported separately as **Ortyo motion**.

## Commands

Human-readable diff:

```bash
cargo run --locked --bin market-trend -- \
  --from docs/market/snapshots/2026-10-01-baseline.yaml \
  --to docs/market/snapshots/2026-10-01-research-1.yaml
```

Machine-readable diff:

```bash
cargo run --locked --bin market-trend -- \
  --from docs/market/snapshots/2026-10-01-baseline.yaml \
  --to docs/market/snapshots/2026-10-01-research-1.yaml \
  --json
```
