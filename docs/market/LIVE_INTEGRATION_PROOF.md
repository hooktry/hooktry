# Hooktry MCIF live integration proof

Checked: 2026-10-02  
DWC: MCIF/Hooktry.15 LIVE-INTEGRATION-PROOF

## Question

DOGFOOD-PROOF proved that synthetic duplicate and ordering probes work.

LIVE-INTEGRATION-PROOF asks a stronger product question:

> Does an Hooktry temporal primitive naturally improve a real integration workflow that exists independently of the proof itself?

The selected workflow is the existing approval notification path:

```text
ApprovalRecord
  -> transactional notification outbox
  -> leased approval webhook provider
  -> HTTP receiver
```

This workflow already exists for CONTROL/NOTIFY product behavior. It was not invented to exercise Scenario.

## Live regression invariant

For one pending approval notification:

```text
notification_id
  -> Idempotency-Key
  -> exactly one approval_requested delivery
  -> 204 receiver response
  -> delivered outbox row
  -> second worker tick finds nothing to claim
```

The Scenario sits around the receiver boundary and asserts:

- operation: `POST /approval`
- exact cardinality: `count = 1`
- normalized `idempotency_key = notification_id`
- response status: `204`
- bounded observation horizon
- quiet/settle window

This means cardinality, idempotency context, and settle semantics are now used as a regression guard for a real Hooktry webhook workflow.

## Acceptance path

CI runs:

```sh
cargo test --locked --test approval_webhook_scenario_dogfood -- --nocapture
```

The test uses production code for:

- ApprovalRecord creation
- transactional outbox creation
- notification claim/lease
- approval webhook payload and headers
- webhook execution
- delivered-state commit
- Scenario exposure
- Interaction capture
- Recording/Replay
- Contract assertion
- cardinality evaluation
- settle-window completion

The only test-specific concession is private-destination allowance in the HTTP executor so the integration can run entirely on loopback CI infrastructure. The provider/outbox state machine is otherwise the same path used by the hosted worker.

## Evidence produced

A successful run demonstrates:

```text
real product workflow
  -> real webhook delivery
  -> normalized idempotency evidence
  -> exact-count assertion
  -> quiet-window proof
  -> durable delivered outbox state
```

The test also calls the worker a second time after successful delivery and requires that no notification can be claimed again.

This is stronger than the synthetic DOGFOOD-PROOF because the temporal primitive is now protecting a workflow with an independent product purpose.

## What is validated

First-party usage evidence now exists for:

- exact cardinality in a real webhook integration
- normalized idempotency matching in a real webhook integration
- settle-window semantics as proof of no immediate extra matching delivery

This is sufficient to keep those primitives as maintained regression infrastructure rather than speculative DSL surface.

## What is not yet validated

This does **not** establish:

- external developer adoption
- willingness to pay
- preference for the current Scenario/Contract syntax
- live ordering demand inside an independent workflow
- need for richer ranged-cardinality syntax

Ordering remains mechanism-proven but not live-integration-proven.

No external-demand signal is created from this test.

## Decision impact

Keep the combined market priority conservative:

```yaml
horizon: validate
decision: differentiation
next_evidence: first_party_usage
```

but reinterpret the remaining evidence gap:

- exact-count/idempotency/settle: first live Hooktry integration proof now exists
- ordering: still needs independent live use
- DSL depth: still needs repeated use, modification pressure, or external feedback

The next valuable step is **not** to add more synthetic variants. It is to leave this proof in the real approval webhook regression path and look for the next naturally occurring use, especially ordering or richer cardinality.

## Promotion trigger

A future promotion should require at least one of:

- the live regression catches a real accidental duplicate during normal development
- another independent Hooktry workflow adopts the same primitive
- another project/user adopts the recipe repeatedly
- a developer changes the thin syntax because the existing primitive is insufficient

Until then, maintain the primitive; do not deepen the DSL.
