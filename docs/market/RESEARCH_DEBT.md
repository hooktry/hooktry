# Hooktry MCIF research debt

Checked: 2026-10-02  
DWC: MCIF/Hooktry.8 RESEARCH-DEBT

The purpose of research debt is **not** to fill every unknown matrix cell.

It answers:

> Which missing observations are most likely to change an Hooktry product, roadmap, positioning, or vector decision?

## Current coverage

The temporal-depth projection now has 290 external product-capability cells:

- 84 evidenced
- 206 unknown

The denominator increased by 60 because six differentiation capabilities were intentionally added to the matrix. This is projection-scope expansion, not lost evidence.

This does **not** mean 171 equally important research tasks remain.

Many unknowns are baseline-completion debt around capabilities Hooktry already implements or around decisions that are already sufficiently established.

## Decision tiers

### P0 - current implementation decision

Research can change the depth or shape of a capability already scheduled for now, and Hooktry is still partial/absent/unknown.

Current P0 capability groups:

1. `search-filter` - 3 unknown / 7 evidenced peers
2. `custom-response` - 3 unknown / 7 evidenced peers
3. `failure-simulation` - 5 unknown / 5 evidenced peers

These are the highest-value checks because they can directly change the current primary-product implementation.

### P1 - next decision with direct demand

Research can change a next-horizon gap and the capability already has direct-demand evidence.

Current P1 groups:

1. `provider-templates` - Hooktry absent; 7 unknown / 3 evidenced peers
2. `signature-verification` - Hooktry unknown; 4 unknown / 6 evidenced peers

These checks can affect whether provider-aware testing becomes the next primary-cohort slice and how generic/provider-specific the adapter model should be.

### P2 - next/validation decision

Research can change an upcoming decision, validate differentiation, or clarify an open Hooktry gap.

Current leading P2 groups include:

- `deterministic-ci` - 5 unknown / 5 evidenced peers; Hookdeck, Webhook Relay, and Beeceptor now show partial CI depth, but not the full Hooktry outcome-semantics contract
- `request-diff` - 7 unknown / 3 evidenced peers; Beeceptor adds partial contract-drift comparison alongside deeper webhooks.cc/Hooklistener evidence
- `persistent-endpoint` - Hooktry partial
- `team-collaboration` - Hooktry partial
- temporal/causal verification capabilities when demand or market evidence makes the differentiation thesis decision-relevant

### P3 - watch/vector decision

Research informs a vector that is intentionally not on the current critical path, for example durable delivery, DLQ, or deeper API virtualization.

### P4 - baseline completion

Useful for market completeness, but unlikely to change the current roadmap. Implemented table stakes such as Hooktry's API, replay, and self-host support belong here even when public demand exists.

This distinction is important: **demand for a capability does not imply more competitor research on that capability has high decision value when Hooktry already covers it and the roadmap decision is closed.**

## Ranking invariant

There is no aggregate numeric score.

Candidates are ordered lexicographically by:

```text
decision-changing tier
-> direct demand
-> Hooktry gap
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

Research debt is not synonymous with competitor research.

A priority declares the evidence source that can resolve its **current** uncertainty:

```text
market_research
  -> first_party_usage
  -> external_usage
```

Other channels such as `implementation` and `watch` are also explicitly non-market research.

Only `next_evidence: market_research` is eligible for the executable top competitor-research queue. Unknown matrix cells stay visible regardless of channel.

Current temporal routing:

- `cardinality` - external usage; 9 external cells remain unknown
- `correlation-context` - external usage; 10 external cells remain unknown
- `observation-window` - external usage; 7 external cells remain unknown
- `ordering` - first-party usage; 9 external cells remain unknown

Therefore:

```text
external_usage:
  3 capabilities / 26 cells

first_party_usage:
  1 capability / 9 cells

total deferred from market research:
  4 capabilities / 35 cells
```

The external-usage routing is justified by two maintained first-party uses:

1. Hooktry's approval outbox/webhook regression proof.
2. Operational's independent cross-repository WebhookProviderAdapter proof using a pinned Hooktry executable.

The next questions are now different:

- **duplicate/idempotency/settle** - will an external developer or pilot adopt and retain this proof shape?
- **ordering** - can an independent first-party workflow naturally require it?
- **DSL depth** - does real use create pressure for richer ranged cardinality/order/context syntax?

More competitor-page reading cannot answer those questions.
