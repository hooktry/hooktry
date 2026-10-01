# Ortyo MCIF trend report

Checked: 2026-10-01

## Current comparison

`2026-10-01-research-1 -> 2026-10-01-research-2`

Both snapshots are on the same date because they represent successive research passes, not a claim of same-day market change.

## Result

- scope products added/removed: 0 / 0
- matrix products added/removed: 0 / 0
- capabilities added/removed: 0 / 0
- matrix research resolutions: 4
- observed external state changes: 0
- atomic observations added: 4
- signals added: 0
- explicit market-motion signals added: 0
- Ortyo capability-status changes: 0
- Ortyo matrix changes: 0
- disposition changes: 0

The four transitions are all `unknown -> partial`.

They are **research resolution**, not market motion.

## What was learned

Three competitors now have evidenced partial deterministic-CI depth:

- Hookdeck - CI authentication and non-interactive webhook forwarding/listening
- Webhook Relay - structured synthetic send plus bounded delivery wait and evidence
- Beeceptor - reproducible version-controlled mock environments with state reset for CI

Beeceptor also has partial structured comparison through OpenAPI contract-drift detection.

None of this currently proves the full Ortyo deterministic outcome model exists elsewhere.

## Cumulative research movement from baseline

`2026-10-01-baseline -> 2026-10-01-research-2`

- research resolutions: 15
- external evidenced cells: 59 -> 74
- external unknown cells: 171 -> 156
- atomic observations: 69 -> 84
- explicit market-motion signals: 0
- Ortyo motion: 0
- disposition changes: 0

## Interpretation boundary

`unknown -> evidenced` remains research resolution.

A true market-motion claim requires dated evidence that a competitor changed, preferably recorded as `class: market_motion`.

Ortyo delivery changes remain a separate `ortyo_motion` channel.

## Commands

Latest incremental diff:

```bash
cargo run --locked --bin market-trend -- \
  --from docs/market/snapshots/2026-10-01-research-1.yaml \
  --to docs/market/snapshots/2026-10-01-research-2.yaml
```

Cumulative diff:

```bash
cargo run --locked --bin market-trend -- \
  --from docs/market/snapshots/2026-10-01-baseline.yaml \
  --to docs/market/snapshots/2026-10-01-research-2.yaml \
  --json
```
