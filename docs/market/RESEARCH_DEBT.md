# Ortyo MCIF research debt

Checked: 2026-10-01  
DWC: MCIF/ORTYO.8 RESEARCH-DEBT

The purpose of research debt is **not** to fill every unknown matrix cell.

It answers:

> Which missing observations are most likely to change an Ortyo product, roadmap, positioning, or vector decision?

## Current coverage

The primary matrix has 230 external product-capability cells:

- 59 evidenced
- 171 unknown

This does **not** mean 171 equally important research tasks remain.

Many unknowns are baseline-completion debt around capabilities Ortyo already implements or around decisions that are already sufficiently established.

## Decision tiers

### P0 - current implementation decision

Research can change the depth or shape of a capability already scheduled for now, and Ortyo is still partial/absent/unknown.

Current P0 capability groups:

1. `search-filter` - 6 unknown / 4 evidenced peers
2. `custom-response` - 5 unknown / 5 evidenced peers
3. `failure-simulation` - 7 unknown / 3 evidenced peers

These are the highest-value checks because they can directly change the current primary-product implementation.

### P1 - next decision with direct demand

Research can change a next-horizon gap and the capability already has direct-demand evidence.

Current P1 groups:

1. `provider-templates` - Ortyo absent; 8 unknown / 2 evidenced peers
2. `signature-verification` - Ortyo unknown; 7 unknown / 3 evidenced peers

These checks can affect whether provider-aware testing becomes the next primary-cohort slice and how generic/provider-specific the adapter model should be.

### P2 - next/validation decision

Research can change an upcoming decision, validate differentiation, or clarify an open Ortyo gap.

Current leading P2 groups include:

- `deterministic-ci` - implemented in Ortyo, but competitor motion and demand proxies can change differentiation claims
- `request-diff` - Ortyo unknown
- `persistent-endpoint` - Ortyo partial
- `team-collaboration` - Ortyo partial
- temporal/causal verification capabilities when demand or market evidence makes the differentiation thesis decision-relevant

### P3 - watch/vector decision

Research informs a vector that is intentionally not on the current critical path, for example durable delivery, DLQ, or deeper API virtualization.

### P4 - baseline completion

Useful for market completeness, but unlikely to change the current roadmap. Implemented table stakes such as Ortyo's API, replay, and self-host support belong here even when public demand exists.

This distinction is important: **demand for a capability does not imply more competitor research on that capability has high decision value when Ortyo already covers it and the roadmap decision is closed.**

## Ranking invariant

There is no aggregate numeric score.

Candidates are ordered lexicographically by:

```text
decision-changing tier
-> direct demand
-> Ortyo gap
-> horizon
-> disposition
-> supporting demand-proxy / market-motion signals
-> evidence scarcity
-> product cohort breadth
-> stable lexical tie-break
```

The tier is the dominant factor.

## Diversity invariant

A naive cell ranking can return ten checks for one capability.

The default queue therefore limits the top list to three products per capability:

```bash
cargo run --locked --bin market-research-debt -- \
  --limit 20 \
  --max-per-capability 3
```

Set `--max-per-capability 0` to disable the cap.

Machine-readable output:

```bash
cargo run --locked --bin market-research-debt -- \
  --limit 20 \
  --max-per-capability 3 \
  --json
```

## Expected first research sprint

With the current model, the first sprint should concentrate on a small set of high-leverage primary competitors across:

1. search/filter depth
2. configurable sender responses
3. failure/latency simulation
4. provider templates
5. signature verification
6. deterministic CI depth
7. structured request diff

Bridge products with several relevant cohorts are intentionally checked early because one observation can inform both the primary job and a depth/adjacent vector.

## Exit rule

Research on a capability can stop before every cell is known when additional observations are unlikely to change:

- its disposition
- current implementation depth
- the next-horizon roadmap
- the differentiation thesis
- a vector activation/stop decision

Unknown is acceptable when it is no longer decision-relevant.
