# Hooktry MCIF cross-project proof

Checked: 2026-10-02  
DWC: MCIF/Hooktry.16 CROSS-PROJECT-PROOF

## Question

LIVE-INTEGRATION-PROOF established that Hooktry's exact-count, idempotency-context, and settle semantics protect a real Hooktry workflow.

CROSS-PROJECT-PROOF asks the next question:

> Does a different product with an independent business purpose choose the same Hooktry primitives to verify its webhook integration?

The consumer is Operational.

## Independent consumer

Operational has its own provider-neutral durable notification architecture. Its webhook provider is not an Hooktry feature.

Operational PR #261 added a real `WebhookProviderAdapter` and a dedicated CI partition that checks out a pinned Hooktry executable:

```text
hooktry/hooktry@9dd0e2597eb22b356c46c2106802d8c41687e1db
```

The projects interact only through process and HTTP boundaries. Operational does not import Hooktry Rust modules.

## Verified path

```text
Operational WebhookProviderAdapter
  -> Idempotency-Key = durable Operational delivery ID
  -> HOOKTRY_EXPOSURE_URL
  -> Hooktry Interaction evidence
  -> receiver
  -> Scenario exact count + idempotency context + settle
```

The receiver independently verifies the Operational payload and delivery/event identity headers, so a passing Scenario cannot hide a malformed provider request.

## CI acceptance

The Operational CI gate produced:

```text
CROSS-PROJECT-PROOF passed:
Operational WebhookProviderAdapter
-> Hooktry Scenario exact-count/idempotency/settle
-> receiver
```

Operational merge commit:

```text
4c8a844d2ab7bacec2ae1f7fc7310e686608ff8b
```

## Evidence interpretation

This is stronger than self-dogfood:

- a second repository owns the integration
- a second product owns the business job
- Hooktry is a pinned external executable
- the boundary is process + HTTP
- the consumer's CI depends on the proof

It is still first-party portfolio evidence, not external customer adoption.

Therefore:

```text
duplicate/idempotency problem       -> public demand proven
Hooktry mechanism                     -> proven
Hooktry live self-use                 -> proven
independent first-party product use -> proven
external adoption                   -> not proven
richer DSL demand                   -> not proven
```

## Decision split

The former combined `cardinality-ordering` hypothesis is now too coarse.

### Duplicate / idempotency / settle

```yaml
horizon: validate
decision: differentiation
next_evidence: external_usage
capabilities: [cardinality, correlation-context, observation-window]
```

The next useful evidence is an external developer or pilot choosing and keeping this proof shape.

### Ordering

```yaml
horizon: validate
decision: differentiation
next_evidence: first_party_usage
capabilities: [ordering]
```

Ordering has a real problem signal and synthetic mechanism proof, but no independent live workflow yet.

## Security finding from the consumer

Operational's webhook adapter is implemented and verified, but its public `kind: webhook` destination API remains intentionally disabled.

Reason: its current destination `config_json` is plaintext, while real webhook URLs commonly contain credentials in path/query values.

This is exactly the kind of boundary evidence cross-project dogfood should surface: Hooktry verified transport behavior without forcing the consuming product to weaken secret handling.

## Stop rule

Do not deepen ranged cardinality, ordering syntax, or the broader Contract DSL because this cross-project proof passed.

For duplicate/idempotency/settle, seek external use.

For ordering, seek an independent first-party live workflow first.
