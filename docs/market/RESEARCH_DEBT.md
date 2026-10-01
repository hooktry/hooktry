# Ortyo MCIF research debt

Checked: 2026-10-01  
DWC: MCIF/ORTYO.8 RESEARCH-DEBT

The purpose of research debt is **not** to fill every unknown matrix cell.

It answers:

> Which missing observations are most likely to change an Ortyo product, roadmap, positioning, or vector decision?

## Current coverage

The temporal-depth projection now has 290 external product-capability cells:

- 84 evidenced
- 206 unknown

The denominator increased by 60 because six differentiation capabilities were intentionally added to the matrix. This is projection-scope expansion, not lost evidence.

This does **not** mean 171 equally important research tasks remain.

Many unknowns are baseline-completion debt around capabilities Ortyo already implements or around decisions that are already sufficiently established.

## Decision tiers

### P0 - current implementation decision

Research can change the depth or shape of a capability already scheduled for now, and Ortyo is still partial/absent/unknown.

Current P0 capability groups:

1. `search-filter` - 3 unknown / 7 evidenced peers
2. `custom-response` - 3 unknown / 7 evidenced peers
3. `failure-simulation` - 5 unknown / 5 evidenced peers

These are the highest-value checks because they can directly change the current primary-product implementation.

### P1 - next decision with direct demand

Research can change a next-horizon gap and the capability already has direct-demand evidence.

Current P1 groups:

1. `provider-templates` - Ortyo absent; 7 unknown / 3 evidenced peers
2. `signature-verification` - Ortyo unknown; 4 unknown / 6 evidenced peers

These checks can affect whether provider-aware testing becomes the next primary-cohort slice and how generic/provider-specific the adapter model should be.

### P2 - next/validation decision

Research can change an upcoming decision, validate differentiation, or clarify an open Ortyo gap.

Current leading P2 groups include:

- `deterministic-ci` - 5 unknown / 5 evidenced peers; Hookdeck, Webhook Relay, and Beeceptor now show partial CI depth, but not the full Ortyo outcome-semantics contract
- `request-diff` - 7 unknown / 3 evidenced peers; Beeceptor adds partial contract-drift comparison alongside deeper webhooks.cc/Hooklistener evidence
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

## Next research sprint

Research sprints 1-2 resolved 15 decision-relevant cells. Temporal-depth pass 3 expanded the decision projection and added 10 evidence-backed states on the new rows. Four P0/P1 checks remain unknown by design; further research should concentrate on the remaining differentiation-depth uncertainty across:

1. temporal/cardinality/ordering semantics
2. correlation/idempotency-aware matching
3. deterministic CI outcome semantics
4. structured interaction diff depth
5. first-party demand evidence for the deeper verification model

Bridge products with several relevant cohorts are intentionally checked early because one observation can inform both the primary job and a depth/adjacent vector.

## Exit rule

Research on a capability can stop before every cell is known when additional observations are unlikely to change:

- its disposition
- current implementation depth
- the next-horizon roadmap
- the differentiation thesis
- a vector activation/stop decision

Unknown is acceptable when it is no longer decision-relevant.


## Evidence channel routing

Research debt is no longer assumed to mean "fill more competitor matrix cells."

A priority can declare:

```yaml
next_evidence: first_party_usage
```

When it does, unresolved competitor cells remain visible in total matrix debt but are excluded from the top external-research checks.

Current temporal validation routing:

- `cardinality` - first-party usage; 9 external cells remain unknown
- `ordering` - first-party usage; 9 external cells remain unknown
- `correlation-context` - first-party usage; 10 external cells remain unknown

Total deferred to first-party usage: **3 capabilities / 28 cells**.

This prevents strong direct demand from accidentally causing endless desk research after the problem itself is already established.

The next evidence for these capabilities should answer:

- do developers run duplicate/order/idempotency scenarios?
- do they rerun or modify them?
- do those scenarios catch failures before production?
- do users need richer cardinality/order/settle syntax than the thin proof provides?
