# Ortyo MCIF trend report

Checked: 2026-10-01

## Status

Temporal baseline established. There is currently one immutable market snapshot:

- `2026-10-01-baseline`

Therefore **no cross-time market trend is claimed yet**. A one-point baseline can describe current evidence, but it cannot establish movement.

## What the next snapshot will measure

`market-trend` will compare the baseline with a later immutable snapshot and report:

- scope products added or removed
- matrix products added or removed
- capabilities added or removed
- capability disposition changes
- Ortyo implementation-status changes
- matrix state transitions
- newly recorded or removed signals
- newly recorded or removed atomic observations
- decision-relevant changes

## Interpretation boundary

A matrix transition is not automatically a market change.

`unknown -> present|partial|absent` is classified as **research resolution** because the earlier snapshot did not know the state.

`present|partial|absent -> unknown` is **research regression** because confidence/evidence was lost.

A transition between two evidenced non-`unknown` states is an **observed state change**. It may support a market-change claim, but the claim should still be tied to dated evidence.

Explicit supply-side change is represented by a newly added signal with:

```yaml
class: market_motion
```

Ortyo changes are reported separately as **Ortyo motion** so product delivery is never confused with competitor movement.

## Commands

Human-readable diff:

```bash
cargo run --locked --bin market-trend -- \
  --from docs/market/snapshots/2026-10-01-baseline.yaml \
  --to docs/market/snapshots/<later>.yaml
```

Machine-readable diff:

```bash
cargo run --locked --bin market-trend -- \
  --from docs/market/snapshots/2026-10-01-baseline.yaml \
  --to docs/market/snapshots/<later>.yaml \
  --json
```

Until a second snapshot exists, CI uses baseline-to-baseline as a zero-delta CLI smoke test while unit tests exercise non-zero synthetic transitions.
