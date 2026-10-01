# Ortyo market model

Checked: 2026-10-01

Ortyo applies the portfolio-wide Market Capability Intelligence Framework (MCIF) from `sergii/projects/market-capability-intelligence`.

## Primary cohort

Ortyo's primary market is **webhook product development, testing, and pilot**.

The first user job is:

> Create a reachable webhook endpoint, observe what a real provider sends, iterate on the receiving integration, replay or reproduce the behavior, and prove that the integration works before or during an early pilot.

This is the anchor for competitor selection. Products that can plausibly replace Ortyo for this job belong in the primary cohort even when their long-term business expands into production delivery, API mocking, or workflow automation.

## Vector invariant

Every Ortyo expansion must strengthen the webhook/integration development lifecycle:

```text
receive / capture
  -> inspect
  -> respond, relay, or replay
  -> verify
```

and preserve canonical Interaction/evidence semantics.

If a capability does not strengthen this lifecycle, it remains an integration, replaceable substrate, or separate product rather than silently redefining Ortyo.

## Cohort interpretation

- **primary** - direct substitutes for webhook development/testing/pilot
- **depth: deterministic verification** - same job, but repeatable contracts, scenarios, CI, and machine-verifiable evidence
- **depth: reliable delivery** - continue from pilot into durable webhook delivery, retries, DLQ, operational controls
- **adjacent: API virtualization** - mock or emulate dependencies and responses around the same integration workflow
- **adjacent: reachability** - tunnels and public exposure used to get traffic to local/private services
- **adjacent: multi-protocol** - expand the same evidence lifecycle beyond HTTP webhooks
- **option: agent control** - approval-controlled external actions and execution around integration work
- **substrate: managed compute** - sandboxes/VMs/workspaces used underneath Ortyo, not product competitors

A product may belong to several cohorts.

## Bridge-product insight

A product spanning the primary cohort and another cohort is **bridge evidence**. It shows that two jobs can coexist in one workflow, but it does not automatically redefine the primary market or justify roadmap expansion.

Beeceptor is the clearest current bridge between Ortyo's primary webhook-development cohort and the adjacent API-virtualization vector: it combines request inspection and history with programmable responses, stateful mocking, proxy/callouts, and failure/latency simulation.

Vercel Labs `emulate` is different: it is an adjacent specialist rather than a primary webhook substitute. It provides local/stateful emulation of third-party APIs, custom emulators, seeds/resets, persistence, and request/state inspection for development, CI, and no-network sandboxes.

The implication is not "become Beeceptor" or "become emulate". The reusable product hypothesis is a **programmable interaction boundary** whose current wedge is webhooks and whose deeper differentiator is deterministic evidence.

See `BOUNDARY_MODEL.md`.

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

Assertions, replay suites, MCP, and CI testing are no longer sufficient differentiation by themselves: current competitors already expose meaningful parts of that surface.

Ortyo's stronger potential differentiation is therefore the **depth of temporal and causal evidence semantics**:

- canonical evidence rather than log-only inspection
- portable Recording/Replay
- exact and ranged cardinality
- durable observed ordering
- explicit observation/settle windows
- correlation, causation, and idempotency context
- persisted assertion evidence and explainable outcomes
- one model shared by HTTP, CLI, MCP, local runtime, and hosted runtime

That hypothesis still requires first-party demand evidence; competitor absence or architectural elegance alone does not prove demand.

## Files

- `scope.yaml` - bounded market, cohort roles, products, stopping rule
- `vectors.yaml` - exploit/explore vectors and activation/stop triggers
- `capabilities.yaml` - normalized capability ontology and initial Ortyo overlay
- `depth-profiles.yaml` - observable dimensions for comparing capability depth
- `personas.yaml` - primary users and jobs-to-be-done
- `scenarios.yaml` - mechanism-independent end-to-end webhook outcomes
- `DEPTH_REPORT.md` - primary-cohort depth comparison and gap analysis
- `BOUNDARY_MODEL.md` - Ortyo's capability topology and bridge-product interpretation
- `signals.yaml` - direct demand, demand proxies, and market-motion evidence
- `DEMAND_REPORT.md` - verification-first demand analysis and contradictory evidence
- `priorities.yaml` - evidence-backed near-term dispositions with confidence
- `matrix.yaml` - current product-by-capability projection with explicit unknowns
- `MATRIX_REPORT.md` - primary-cohort completeness and sharpened differentiation analysis
- `snapshots/` - immutable, self-contained MCIF market-state snapshots for temporal comparison
- `competitors.yaml` - product positioning and authoritative sources
- `observations/seed.yaml` - initial atomic market evidence

## Current decision

Public demand evidence now supports the primary capture/inspect/replay/local-development loop and gives moderate support to deterministic CI outcomes. It does not yet directly validate Ortyo's full Contract/Scenario/cardinality/ordering surface.

The current sequencing rule is therefore:

1. complete primary webhook inspection and replay ergonomics
2. add narrow response/failure controls
3. add provider-aware signatures/templates
4. expose a thin deterministic CI proof
5. collect first-party usage before deepening verification DSLs or optional vectors

See `DEMAND_REPORT.md` and `priorities.yaml`.

## Next passes

1. Complete remaining atomic product-capability evidence in the primary cohort.
2. Reduce only decision-relevant unknown cells in the current capability matrix.
3. Record pricing/retention/limits separately because these are volatile.
4. Compare future research passes against the immutable 2026-10-01 baseline snapshot.
5. Start collecting first-party demand proxies from Ortyo dogfooding/public usage.


## Executable validation

The market model is checked in CI:

```bash
cargo run --locked --bin market-model-check -- --check
```

The validator treats `matrix.yaml` as a projection over atomic evidence rather than an independent source of truth.

Current invariants include:

- capability, product, cohort, scenario, signal, priority, and vector references must resolve
- capability dispositions and Ortyo implementation states use the canonical vocabularies
- matrix states are limited to `present | partial | absent | unknown`
- Ortyo matrix cells must agree with `capabilities.yaml`
- every external non-`unknown` matrix cell requires a current atomic observation with the same state
- observations require date, applicability, freshness, confidence, assertion, and source URL
- direct-demand signals require an explicit `supports | contradicts | mixed` direction

Missing evidence is represented as `unknown`, not inferred as absence.


## Snapshot discipline

Market snapshots live under `docs/market/snapshots/` and are append-only.

The baseline is `2026-10-01-baseline.yaml`. A snapshot is self-contained enough to compare future market states without reconstructing old Git trees: it records scope products, matrix products, capability dispositions and Ortyo status, the evidence-backed matrix, signal metadata, and atomic observation metadata.

Validation:

```bash
cargo run --locked --bin market-snapshot-check -- --check
```

CI enforces two separate invariants:

- snapshot contents must be internally consistent and use the MCIF vocabularies
- an existing snapshot file may not be modified, renamed, or deleted; later research creates a new snapshot

The snapshot's `source_commit` is provenance, not a mutable pointer to the current repository state.
