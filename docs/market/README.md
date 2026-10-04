# Hooktry market model

Checked: 2026-10-04

Hooktry applies the portfolio-wide Market Capability Intelligence Framework (MCIF) from `sergii/projects/market-capability-intelligence`.

## Primary cohort

Hooktry's primary market is **webhook product development, testing, and pilot**.

The first user job is:

> Create a reachable webhook endpoint, observe what a real provider sends, iterate on the receiving integration, replay or reproduce the behavior, and prove that the integration works before or during an early pilot.

This is the anchor for competitor selection. Products that can plausibly replace Hooktry for this job belong in the primary cohort even when their long-term business expands into production delivery, API mocking, or workflow automation.

## Vector invariant

Every Hooktry expansion must strengthen the webhook/integration development lifecycle:

```text
receive / capture
  -> inspect
  -> respond, relay, or replay
  -> verify
```

and preserve canonical Interaction/evidence semantics.

If a capability does not strengthen this lifecycle, it remains an integration, replaceable substrate, or separate product rather than silently redefining Hooktry.

## Cohort interpretation

- **primary** - direct substitutes for webhook development/testing/pilot
- **depth: deterministic verification** - same job, but repeatable contracts, scenarios, CI, and machine-verifiable evidence
- **depth: reliable delivery** - continue from pilot into durable webhook delivery, retries, DLQ, operational controls
- **adjacent: API virtualization** - mock or emulate dependencies and responses around the same integration workflow
- **adjacent: reachability** - tunnels and public exposure used to get traffic to local/private services
- **adjacent: multi-protocol** - expand the same evidence lifecycle beyond HTTP webhooks
- **option: agent control** - approval-controlled external actions and execution around integration work
- **substrate: managed compute** - sandboxes/VMs/workspaces used underneath Hooktry, not product competitors

A product may belong to several cohorts.

## Bridge-product insight

A product spanning the primary cohort and another cohort is **bridge evidence**. It shows that two jobs can coexist in one workflow, but it does not automatically redefine the primary market or justify roadmap expansion.

Beeceptor is the clearest current bridge between Hooktry's primary webhook-development cohort and the adjacent API-virtualization vector: it combines request inspection and history with programmable responses, stateful mocking, proxy/callouts, and failure/latency simulation.

Vercel Labs `emulate` is different: it is an adjacent specialist rather than a primary webhook substitute. It provides local/stateful emulation of third-party APIs, custom emulators, seeds/resets, persistence, and request/state inspection for development, CI, and no-network sandboxes.

The implication is not "become Beeceptor" or "become emulate". The reusable product hypothesis is a **programmable interaction boundary** whose current wedge is webhooks and whose deeper differentiator is deterministic evidence.

See `BOUNDARY_MODEL.md` and `PRODUCT_THESIS_V1.md`.

## Primary products in research pass 1

Material representatives:

- Webhook.site
- Hookdeck
- Webhook Relay
- Svix / Svix Play
- Beeceptor
- webhooks.cc
- Hooklistener
- Webhooker
- Hook0 / Hook0 Play
- RequestBin.net

Long-tail and utility products remain discoverable but do not need equal research depth once new entrants stop adding capabilities relevant to the current product decision.

## Initial finding

The direct market has moved beyond "give me a URL and show the JSON".

Material competitors now combine subsets of:

- instant/public endpoints
- live inspection and history
- custom responses and failure simulation
- replay/forwarding to localhost
- persistent URLs
- filtering, transformations, and routing
- retries, durable queues, DLQ, and delivery observability
- API/CLI automation
- provider templates and signature verification
- MCP/agent surfaces
- CI-oriented testing

The agent surface itself is therefore not a durable differentiator. Hookdeck, Webhook Relay, webhooks.cc, Hook0, Svix and others already expose agent-oriented workflows.

Assertions, replay suites, MCP, CI wiring, bounded waits, reproducible test environments, and structured evidence are no longer sufficient differentiation by themselves: current competitors already expose meaningful parts of that surface.

Temporal-depth research shows competitors already cover reusable suites, request-local contracts, expected counts, timestamp-sorted multi-event capture, and bounded waits. Hooktry's remaining potential differentiation is therefore the **deeper temporal and causal evidence semantics**:

- canonical evidence rather than log-only inspection
- portable Recording/Replay
- ranged cardinality with proof that no later extra match violates the expectation
- durable observed-order assertions
- hard horizons plus explicit quiet/settle windows
- normalized correlation, causation, request, message, trace, and idempotency context
- persisted assertion evidence and explainable outcomes
- one model shared by HTTP, CLI, MCP, local runtime, and hosted runtime

Public production incidents now validate duplicate, ordering, and idempotency failure classes directly, but the exact Hooktry temporal/causal DSL still requires first-party usage evidence; problem validity does not automatically validate the abstraction.

## Files

- `scope.yaml` - bounded market, cohort roles, products, stopping rule
- `vectors.yaml` - exploit/explore vectors and activation/stop triggers
- `capabilities.yaml` - normalized capability ontology and initial Hooktry overlay
- `depth-profiles.yaml` - observable dimensions for comparing capability depth
- `personas.yaml` - primary users and jobs-to-be-done
- `scenarios.yaml` - mechanism-independent end-to-end webhook outcomes
- `DEPTH_REPORT.md` - primary-cohort depth comparison and gap analysis
- `BOUNDARY_MODEL.md` - Hooktry's capability topology and bridge-product interpretation
- `PRODUCT_THESIS_V1.md` - active product thesis after S003/S004/S006/S007: core job, invariant, P1/P2, and explicit non-goals
- `signals.yaml` - direct demand, demand proxies, and market-motion evidence
- `DEMAND_REPORT.md` - verification-first demand analysis and contradictory evidence
- `priorities.yaml` - evidence-backed near-term dispositions with confidence
- `matrix.yaml` - current product-by-capability projection with explicit unknowns
- `MATRIX_REPORT.md` - primary-cohort completeness and sharpened differentiation analysis
- `snapshots/` - immutable, self-contained MCIF market-state snapshots for temporal comparison
- `TREND_REPORT.md` - current temporal-analysis status and trend interpretation rules
- `RESEARCH_DEBT.md` - decision-value research queue over unresolved matrix evidence
- `RESEARCH_SPRINT_2026-10-01.md` - first executed P0/P1 evidence sprint and decision impact
- `RESEARCH_SPRINT_2_2026-10-01.md` - second differentiation-depth research sprint
- `TEMPORAL_DEPTH_REPORT.md` - cardinality/order/window/correlation depth analysis
- `DEMAND_PROOF_REPORT.md` - direct production evidence for duplicate/order/idempotency failure classes
- `FIRST_PARTY_PROOF.md` - thin product evidence loop for duplicate/order temporal primitives
- `DOGFOOD_PROOF.md` - end-to-end buggy/fixed temporal probe acceptance and its evidence limits
- `LIVE_INTEGRATION_PROOF.md` - real approval-outbox webhook workflow protected by Scenario exact-count/idempotency/settle semantics
- `CROSS_PROJECT_PROOF.md` - Operational consumes a pinned Hooktry executable across repository/process/HTTP boundaries
- `EPHEMERAL_HOOK_PROOF.md` - manual production proof of anonymous Hook ingest -> persistence -> live WEB1 inspection
- `usage-evidence.yaml` - structured self-dogfood, first-party portfolio, and future external-customer usage evidence
- `competitors.yaml` - product positioning and authoritative sources
- `observations/seed.yaml` - initial atomic market evidence

## Current decision

S004/S007 sharpen the product decision beyond the earlier desk-research sequence.

The active thesis is now:

> Hooktry is a programmable public interaction boundary that turns external callbacks into canonical, reproducible proof for humans, CI, and software agents.

Near-term sequencing:

1. preserve the instant anonymous Hook / inspection fast path
2. complete replay and development forwarding ergonomics
3. expose a small generic `wait_for_interaction` synchronization primitive over canonical Interaction evidence
4. add narrow first-class Response Policy semantics
5. productize Contract/Scenario/Outcome as the richer proof path
6. add provider-aware verification/signing/re-signing as the next fidelity layer
7. keep production retry/DLQ/fan-out infrastructure outside core until users explicitly pull Hooktry into that job

Agent support is a first-class actor/interface property, not the category definition.

See `PRODUCT_THESIS_V1.md`, `DEMAND_REPORT.md`, and `priorities.yaml`.

## Next passes

1. Complete remaining atomic product-capability evidence in the primary cohort.
2. Reduce only decision-relevant unknown cells in the current capability matrix.
3. Record pricing/retention/limits separately because these are volatile.
4. Compare future research passes against the immutable 2026-10-01 baseline snapshot.
5. Seek an external developer/pilot for exact-count/idempotency/settle, find an independent first-party live ordering use, and require real syntax pressure before deepening the temporal DSL.


## Executable validation

The market model is checked in CI:

```bash
cargo run --locked --bin market-model-check -- --check
```

The validator treats `matrix.yaml` as a projection over atomic evidence rather than an independent source of truth.

Current invariants include:

- capability, product, cohort, scenario, signal, priority, and vector references must resolve
- capability dispositions and Hooktry implementation states use the canonical vocabularies
- matrix states are limited to `present | partial | absent | unknown`
- Hooktry matrix cells must agree with `capabilities.yaml`
- every external non-`unknown` matrix cell requires a current atomic observation with the same state
- observations require date, applicability, freshness, confidence, assertion, and source URL
- direct-demand signals require an explicit `supports | contradicts | mixed` direction

Missing evidence is represented as `unknown`, not inferred as absence.


## Snapshot discipline

Market snapshots live under `docs/market/snapshots/` and are append-only.

The baseline is `2026-10-01-baseline.yaml`. A snapshot is self-contained enough to compare future market states without reconstructing old Git trees: it records scope products, matrix products, capability dispositions and Hooktry status, the evidence-backed matrix, signal metadata, and atomic observation metadata.

Validation:

```bash
cargo run --locked --bin market-snapshot-check -- --check
```

CI enforces two separate invariants:

- snapshot contents must be internally consistent and use the MCIF vocabularies
- an existing snapshot file may not be modified, renamed, or deleted; later research creates a new snapshot

The snapshot's `source_commit` is provenance, not a mutable pointer to the current repository state.


## Temporal analysis

Compare any two immutable snapshots with:

```bash
cargo run --locked --bin market-trend -- \
  --from docs/market/snapshots/<older>.yaml \
  --to docs/market/snapshots/<newer>.yaml
```

Use `--json` for a machine-readable report.

The analyzer deliberately separates:

- **research resolution** - an `unknown` matrix cell becomes evidenced; this is new knowledge, not proof the competitor changed
- **research regression** - a previously evidenced cell becomes `unknown`
- **observed state change** - two non-`unknown` external states differ across snapshots
- **market motion signal** - an explicitly recorded signal with `class: market_motion`
- **Hooktry motion** - Hooktry implementation or matrix state changes
- **decision impact** - changes touching in-scope capability dispositions or newly recorded direct-demand/market-motion signals

Do not call a matrix evidence-resolution event "market motion" unless independent evidence establishes that the product itself changed.


## Research debt

Do not optimize for matrix completeness. Rank missing evidence by whether resolving it can change a product decision:

```bash
cargo run --locked --bin market-research-debt -- \
  --limit 20 \
  --max-per-capability 3
```

Use `--json` for automation.

The queue separates current/next/validation/vector decisions from baseline-completion debt. An already implemented table stake does not become high-priority research merely because demand evidence exists.

See `RESEARCH_DEBT.md`.


## Evidence channel

A product priority can choose the next evidence source explicitly.

```yaml
next_evidence: first_party_usage
```

This is used when desk research has established the problem sufficiently and the remaining uncertainty concerns Hooktry's own abstraction or workflow.

`market-research-debt` keeps those unknown competitor cells visible in coverage totals but removes them from the top external-research queue.
