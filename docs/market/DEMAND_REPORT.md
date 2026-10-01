# Ortyo MCIF demand report

Checked: 2026-10-01  
DWC: MCIF/ORTYO.3 DEMAND

This pass asks a different question from the competitor matrix:

> Do developers show evidence of wanting the jobs Ortyo is trying to solve, and which parts of the verification-first thesis are directly supported versus inferred?

Competitor presence is not counted as demand. Signals remain separated into direct demand, demand proxies, and market motion.

## What is directly supported

### Capture + replay + local iteration - SUPPORTED

Multiple public developer discussions independently describe the cost of waiting for real provider events, fragile/expiring tunnels, missing request/response history, and the desire to capture once and replay against local code.

This supports the primary loop:

```text
receive real event
  -> retain faithful evidence
  -> inspect
  -> replay locally
  -> repeat
```

This is not unique to Ortyo. It is a validated market job and therefore table stakes.

### Headless/API access - SUPPORTED

Public QA discussion explicitly asks for a webhook testing service whose captured requests can be pulled through an API.

This reinforces Ortyo's choice to keep HTTP/CLI/MCP surfaces over the same model rather than treating the browser UI as the product authority.

### Signature fidelity and provider-aware cases - SUPPORTED

Recent public integration issues show real failures around provider-specific and Standard Webhooks signature formats. Signature behavior is byte-sensitive and provider conventions differ.

This supports provider adapters/templates and signature helpers, but it does **not** support hard-coding provider semantics into the canonical Interaction model.

### Local-first / self-hostable operation - SUPPORTED

One explicit counter-signal against a proposed hosted webhook-testing product said the behavior was useful but dependence on another remote service was unacceptable; a local proxy/tool was preferred.

This matches Ortyo's architecture well:

```text
local / OSS core
  + optional hosted Exposure
  + optional durable cloud services
```

Local-first should be treated as a product property, not an accidental deployment mode.

## What is supported at the problem level

### Async deterministic waiting - STRONG PROBLEM EVIDENCE, QUIET-PERIOD DEMAND UNPROVEN

Webhook flows are asynchronous. GitHub documents that deliveries can take minutes to arrive and can be throttled under surge conditions.

There is credible support for:

- bounded waits/timeouts
- waiting for evidence or outcome
- machine-readable failure when the expected event never arrives

What remains unproven is the stronger Ortyo-specific primitive: waiting beyond the first match for a **quiet/settle period** to prove that no later duplicate or extra event invalidates the expectation.

So the hard-horizon problem is validated; quiet-period demand is still a hypothesis.

### Duplicate / out-of-order / idempotency behavior - STRONG DIRECT PROBLEM EVIDENCE

The evidence is now materially stronger than generic provider guidance.

Recent public production bug reports independently show:

- one GitHub event executing a workflow twice because duplicate deliveries were not deduplicated
- duplicate Stripe deliveries sending cancellation/payment-failed notifications twice because the processed-event check was not atomic
- out-of-order WhatsApp status webhooks regressing a message from delivered back to sent
- Stripe entitlement races where event ordering and stale events can persist the wrong subscription state
- webhook contracts whose delivery identity is too weak or scoped incorrectly for safe deduplication

Provider documentation independently confirms the same underlying realities: Shopify warns that duplicate deliveries can occur and provides a stable webhook ID for deduplication, while Shopify and GitHub both document that webhook order/timing is not guaranteed.

This strongly supports the **failure classes** behind Ortyo cardinality, ordering, and correlation/idempotency context.

It still does **not prove** demand for Ortyo's exact abstractions such as ranged cardinality syntax, durable-order predicates, quiet/settle windows, or a Contract/Scenario DSL.

That distinction remains explicit.

### Deterministic CI integration proof - MODERATE SUPPORT

Svix reported internal and customer usage of its Play API for automated webhook tests. Current independent engineering guidance argues for deterministic outcome assertions rather than merely checking a 200 response. webhooks.cc has also invested directly in signed CI fixtures, waits, and cleanup.

So the direction:

```text
webhook traffic -> evidence -> deterministic CI result
```

has credible support.

But public evidence currently supports the **outcome**, not specifically Ortyo's full implementation shape.

## What is not directly validated yet

The following remain product hypotheses despite being technically coherent:

- a first-class Contract DSL as the preferred user abstraction
- explicit cardinality assertions as a feature users seek by name
- explicit ordering assertions as a feature users seek by name
- Scenario as the preferred packaging abstraction
- approval-gated agent actions as part of webhook development
- generic managed compute attached to webhook workflows
- multi-protocol expansion beyond concrete webhook-adjacent requests

For these, absence of direct demand is not evidence of absence. It means we should not use the competitor matrix as justification to accelerate them.

## Important contradictory evidence

### Provider-native CLIs are real substitutes

Feedback on a Stripe-focused webhook testing idea challenged the value proposition because Stripe CLI already triggers and forwards test events.

Implication:

> Ortyo should not try to beat every provider's native CLI at provider-specific event generation.

Its advantage must come from a **cross-provider, persistent, evidence-backed workflow** that remains useful when the provider has no good sandbox or CLI.

### Hosted-only is a negative for some developers

The same discussion contains a clear preference for a local tool rather than another remote dependency.

Implication:

> Cloud should add reachability, durability, sharing, and continuity - not become a prerequisite for the core development/testing loop.

### Simple capture/replay is often enough

Some developer workflows are satisfied by:

```text
webhook.site / provider CLI / tunnel
  + saved payload
  + curl/test fixture
```

That is healthy competitive pressure. Ortyo must earn its extra complexity by making verification measurably easier and more deterministic.

## Verification-first thesis: current status

### Supported

- canonical retained evidence is useful
- faithful capture/replay is useful
- async waiting is useful
- signatures/provider fidelity matter
- duplicates/out-of-order behavior are real failure modes
- CI automation is a real workflow
- local-first operation matters to at least part of the developer market

### Still a hypothesis

- users will prefer one integrated evidence/Contract/Scenario model over simpler scripts and provider CLIs
- cardinality + ordering assertions are compelling enough to drive adoption
- the same users will carry Ortyo from development into production delivery
- agent approval/control belongs inside the same product rather than as an optional integration

## Product implication

The evidence argues for a narrow sequencing rule:

```text
FIRST
  make the primary webhook loop excellent

THEN
  expose Ortyo verification primitives exactly where they remove known pain

ONLY THEN
  deepen optional vectors when direct usage creates a trigger
```

Concretely:

1. Mature the human inspector.
2. Make capture -> replay local iteration trivial.
3. Add configurable response/failure behavior.
4. Keep the same workflow fully API/CLI accessible.
5. Add provider template/signature adapters.
6. Package observation windows + machine-readable outcomes into a very small CI proof.
7. Dogfood cardinality/order/Scenario and collect direct demand before expanding their surface area.
8. Preserve local-first/self-hosted operation while using Cloud for convenience and durability.

## Demand gaps to measure next

We need product-generated evidence, not more desk research, for:

- percentage of Hooks that are replayed
- time from first capture to first replay
- repeated use of the same Hook across sessions
- CLI/API/MCP versus browser usage
- number of Scenario runs per active developer
- frequency of duplicate/cardinality failures
- frequency of order violations
- use of correlation/idempotency matching
- provider distribution
- requests for stable URLs/retention
- requests for retries/DLQ/production reliability
- explicit requests for approval-controlled actions

These should become Ortyo's first-party demand proxies once dogfooding/public usage exists.
