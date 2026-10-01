# Market snapshots

This directory is an append-only history of Ortyo's MCIF market state.

Each snapshot is self-contained and captures enough normalized state to compare research passes over time:

- bounded scope products and the smaller matrix product set
- capability disposition and Ortyo implementation overlay
- evidence-backed product-by-capability matrix
- market and demand signal metadata
- atomic observation metadata
- source dates and the Git commit used as provenance

## Rules

1. Never edit, rename, or delete a merged snapshot.
2. Correct later knowledge by recording new observations and creating a new snapshot.
3. `unknown` means not established by current evidence; it must not be rewritten to `absent` without evidence.
4. A newer snapshot may add products, capabilities, signals, and observations or change current projections as evidence changes.
5. Trend claims must cite at least two immutable snapshots.

CI validates snapshot structure with:

```bash
cargo run --locked --bin market-snapshot-check -- --check
```

and rejects non-additive changes under this directory.
