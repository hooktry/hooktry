# Programmable interaction boundary model

Checked: 2026-10-01

This document refines Ortyo's product topology without changing the primary cohort.

The primary cohort remains **webhook product development, testing, and pilot**. The model below describes how current and possible future capabilities relate to the same boundary primitive.

## Product hypothesis

Ortyo is not defined as a generic mock server, tunnel, event gateway, or compute platform.

The working hypothesis is:

> Ortyo is a programmable interaction boundary that can receive, respond, forward, emulate, observe, and prove interactions while preserving one canonical Interaction/evidence model.

The current public product wedge is the webhook-specific **Hook**. Hook remains a specialized form of the broader **Exposure** primitive.

## Capability topology

```text
ORTYO
|
|-- RECEIVE
|   |-- ephemeral HTTP endpoint
|   |-- webhook receiver
|   `-- request bin
|
|-- RESPOND
|   |-- static response
|   |-- declarative rules
|   |-- dynamic response
|   |-- stateful response
|   `-- failure / latency behavior
|
|-- FORWARD
|   |-- relay
|   |-- proxy / callout
|   |-- transform
|   `-- retry
|
|-- EMULATE
|   |-- arbitrary HTTP API
|   |-- OpenAPI-shaped service
|   |-- provider-specific emulator
|   `-- stateful service
|
|-- OBSERVE
|   |-- request / response
|   |-- timeline
|   |-- state changes
|   `-- correlation
|
`-- PROVE
    |-- retained history
    |-- replay
    |-- assertions
    |-- contracts
    |-- structured diffs
    `-- evidence bundle
```

These are capability layers, not six independent products and not six roadmap commitments.

## Market interpretation

### Primary wedge

Webhook.site, Hookdeck, Webhook Relay, Svix Play, Beeceptor, webhooks.cc, Hooklistener, Webhooker, Hook0 Play, and RequestBin-style products can substitute for some or all of the primary webhook development/testing job.

The near-term product sequence therefore remains centered on RECEIVE + OBSERVE + replay/response ergonomics, followed by thin deterministic PROVE.

### Beeceptor as a bridge product

Beeceptor spans the primary webhook-development job and the adjacent API-virtualization job.

Its documented model combines:

- traffic inspection and searchable request history
- declarative and dynamic mock responses
- stateful API behavior
- proxying/callouts to upstream services
- controlled latency and failure simulation
- multiple API protocols and contract-shaped endpoints

This is evidence that RESPOND, FORWARD, EMULATE, and OBSERVE can form one coherent developer workflow. It is not, by itself, evidence that Ortyo should immediately expand into full API virtualization.

### Vercel Labs emulate as an adjacent specialist

Vercel Labs `emulate` focuses on local/stateful substitutes for third-party APIs. It provides built-in and custom emulators, seeds/resets, persistence, and request/state inspection, aimed at development, CI, and no-network sandboxes.

It is useful evidence for the **shape** of the API-virtualization vector, but it is not a primary webhook substitute.

## Ortyo differentiation hypothesis

The market already demonstrates RECEIVE, RESPOND, FORWARD, EMULATE, and OBSERVE in many combinations.

Ortyo should not assume that combining those verbs is sufficient differentiation.

The stronger hypothesis is that Ortyo can make the boundary **provable**:

```text
capture reality
  -> preserve canonical evidence
  -> replay or emulate deterministically
  -> assert contracts / ordering / cardinality
  -> produce machine-readable proof
```

The same Interaction/evidence model should survive across HTTP, CLI, MCP, local runtime, and managed runtime.

## Vector rule

A bridge product changes confidence in a vector, not the cohort definition.

Use this rule:

1. Keep the primary cohort anchored to the first user job and lifecycle stage.
2. Treat products spanning primary + depth/adjacent cohorts as bridge evidence.
3. Ask whether the adjacent job reuses the same users, workflow, primitives, and evidence.
4. Promote the vector only when first-party usage or direct demand satisfies its `deepen_if` / `expand_if` trigger.
5. If the capability does not preserve the vector invariant, keep it as an integration, replaceable substrate, or separate product.

For Ortyo today:

- deterministic verification is the strongest depth vector
- reliable delivery is a later depth vector
- API virtualization is a credible adjacent vector strengthened by Beeceptor and emulate
- multi-protocol remains adjacent
- agent control remains an option
- managed compute remains replaceable substrate/option infrastructure

## Consequence for implementation

Do not build a second independent "mock server" domain model.

If API virtualization is activated later, it should extend Exposure/Interaction/evidence semantics rather than create a parallel product core. Stateful emulation can add domain-specific state and behavior, but request/response evidence, correlation, replay, contracts, and proof should remain shared.
