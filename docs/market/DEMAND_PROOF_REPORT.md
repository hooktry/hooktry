# Ortyo MCIF demand proof: temporal and causal failures

Checked: 2026-10-01  
DWC: MCIF/ORTYO.12 DEMAND-PROOF

## Question

Temporal-depth research found a narrow capability gap around:

- ranged cardinality
- durable observed ordering
- hard horizon plus quiet/settle window
- normalized correlation/causation/idempotency context
- persisted assertion evidence
- behavior-mismatch versus infrastructure/tool outcomes

This pass asks a different question:

> Are duplicate, ordering, and identity failures painful enough in real systems to justify continued product validation, or is the remaining Ortyo moat only technically elegant?

## Direct production evidence

### Duplicate processing is a real failure class

Independent public bug reports show duplicate webhook deliveries causing real duplicate work:

- one GitHub trigger executing a workflow twice because duplicate deliveries were processed without an ingest idempotency guard
- close-together duplicate Stripe deliveries both passing a non-atomic processed-event check and sending notifications twice

These are not hypothetical resilience exercises. They are concrete cases where the desired logical cardinality was effectively:

```text
one provider event
-> one logical side effect
```

This supports the **problem behind cardinality assertions**.

It does not by itself prove that users want an `exactly(1)` or `at_most(1)` DSL.

### Out-of-order delivery causes state corruption

Independent production issues show ordering failures causing wrong state:

- WhatsApp status webhooks arriving out of order and regressing a persisted/UI state from `delivered` back to `sent`
- Stripe entitlement events racing across transient and stale states and leaving the wrong subscription state

This validates ordering as an application-correctness problem, not merely an observability concern.

It does not yet prove demand for a first-class ordering assertion language.

### Delivery identity and idempotency context are subtle

Two different bug classes show why "just dedupe by ID" is not enough:

1. a webhook emitter generated a fresh event ID for repeated emission of one logical event, making downstream deduplication impossible
2. a consumer deduplicated globally by provider delivery ID and accidentally suppressed a legitimate second configured route

This supports the need to preserve and reason about **identity in context**:

```text
provider delivery identity
+ logical event identity
+ route / endpoint / consumer scope
+ causation / request context
```

That is much closer to Ortyo's `correlation-context` primitive than generic logging is.

## Provider-level demand proxies

Provider documentation independently confirms the same problem space.

Shopify explicitly documents that duplicate deliveries may occur after network timeouts or retries and recommends durable deduplication with `X-Shopify-Webhook-Id`.

Shopify also states that ordering is not guaranteed, even for events concerning the same resource.

GitHub documents that deliveries can be delayed by minutes and throttled under load, and separately documents out-of-order delivery.

These sources validate that duplicate/order/timing failures are normal properties of webhook systems rather than rare defects in a few applications.

## What is now validated

### Strongly supported problem family

- duplicate processing
- exactly-once logical side-effect expectations
- out-of-order state transitions
- stable delivery identity for deduplication
- correct scoping of idempotency identity
- variable and delayed delivery timing

### Still not directly validated as a product abstraction

- `exactly / at_least / at_most` syntax
- a first-class durable-order predicate language
- quiet/settle windows after the first successful match
- a unified correlation/causation/idempotency matching DSL
- Contract/Scenario as the abstraction users want to author
- willingness to pay specifically for these deeper primitives

## Priority impact

The `cardinality-ordering` decision remains:

```text
horizon: validate
decision: differentiation
```

but confidence rises from **low -> medium**.

Reason:

- problem evidence is now direct and repeated
- provider behavior independently supports the same failure classes
- exact Ortyo abstraction demand remains unproven

This is intentionally **not** a roadmap promotion to `next`.

## Observation-window conclusion

Hard waiting horizons are well supported because delivery is asynchronous and variably delayed.

The stronger primitive:

> "the expectation matched; now remain quiet for N more milliseconds to prove no duplicate/extra event arrives"

is still a product hypothesis.

A quiet/settle window is technically well motivated by the duplicate evidence, but no direct source found in this pass explicitly asks for that abstraction.

## Product implication

The evidence justifies continuing to validate the narrow temporal/causal moat, but not expanding the DSL blindly.

The next product evidence should come from **first-party usage**.

Recommended thin proof:

1. provide one duplicate/idempotency scenario recipe
2. provide one out-of-order scenario recipe
3. expose machine-readable outcome evidence
4. instrument whether users actually run, repeat, or modify these scenarios
5. record whether failures are found before production
6. only then deepen cardinality/order/settle/correlation syntax

The target question changes from:

> "Can Ortyo model this elegantly?"

to:

> "Do developers repeatedly use Ortyo to catch these failures before production?"

## Stop condition

Do not deepen the temporal DSL merely because more public bug reports can be found.

Desk research can now stop for the **existence of the problem**.

Further research should focus on either:

- evidence that another product already solves the remaining job deeply, invalidating differentiation
- first-party Ortyo usage that validates the product abstraction

