# Ortyo MCIF research sprint 2

Checked: 2026-10-01  
DWC: MCIF/ORTYO.10 RESEARCH-2

## Goal

Finish the highest-value unresolved P0/P1 checks where possible, then test the P2 differentiation boundary around deterministic CI and structured comparison.

## Unresolved P0/P1 checks

The sprint re-checked:

- Hook0 / custom-response
- Hookdeck / failure-simulation
- Hooklistener / provider-templates
- Webhook Relay / provider-templates

No new current official evidence was strong enough to resolve those four cells without inference.

They remain `unknown`.

This is intentional: a missing product page or unsuccessful search is not evidence of absence.

## P2 deterministic CI depth

Resolved:

- Hookdeck / deterministic-ci -> `partial`
- Webhook Relay / deterministic-ci -> `partial`
- Beeceptor / deterministic-ci -> `partial`

### Hookdeck

Hookdeck has explicit CI authentication and non-interactive webhook forwarding/listening through its CLI.

That is useful CI automation, but the researched official surfaces do not establish Ortyo-like machine-readable assertion reports plus stable exit semantics that distinguish behavior mismatch from infrastructure/tool failure.

### Webhook Relay

Webhook Relay exposes a strong agentic test loop:

`send_webhook -> wait_for_webhook_log -> inspect transformed request / destination response`

The wait primitive is bounded and produces explicit settled/delivery statuses.

That is materially closer to deterministic webhook testing, but the researched evidence still does not establish a general assertion DSL and stable behavior-vs-infrastructure CI outcome contract.

### Beeceptor

Beeceptor explicitly supports reproducible CI environments through version-controlled mock rules, complete configuration replacement, state reset, and GitHub Actions usage.

This is deterministic environment setup, not the same thing as deterministic evidence/outcome semantics.

## P2 structured comparison

Resolved:

- Beeceptor / request-diff -> `partial`

Beeceptor compares observed request/response traffic against an OpenAPI baseline and surfaces contract drift.

That is a useful structural comparison primitive, but it is narrower than a general captured-attempt or baseline-vs-candidate interaction diff.

Still unknown:

- Hookdeck / request-diff
- Webhook Relay / request-diff

The researched official material exposes inspection, transformed requests, replay, and logs, but not a first-class structural comparison feature equivalent to the capability definition.

## Decision impact

No disposition changed.

No Ortyo implementation status changed.

No explicit market-motion signal was added.

The sprint produced four research resolutions:

- external evidenced cells: 70 -> 74
- external unknown cells: 160 -> 156
- atomic observations: 80 -> 84
- matrix research resolutions: 4
- observed external state changes: 0
- Ortyo motion: 0

## Differentiation impact

The evidence narrows the claim further:

> CI integration, non-interactive execution, bounded waits, reproducible environments, and structured request evidence are competitive capabilities. They are not sufficient differentiation.

The stronger Ortyo hypothesis remains the combination of:

- deterministic behavior-vs-infrastructure outcome semantics
- contracts over canonical interaction evidence
- ranged cardinality
- durable observed ordering
- explicit observation/settle windows
- correlation/causation/idempotency context
- persisted assertion evidence
- explainable multi-event outcomes

The next useful research question is no longer "do competitors work in CI?" It is:

> Do competitors expose temporal and causal assertion semantics deep enough to invalidate this narrower Ortyo differentiation thesis?

## Next research slice

Prioritize:

1. temporal/cardinality/ordering semantics in webhooks.cc and Hooklistener
2. correlation/idempotency-aware matching in primary competitors
3. request-diff depth beyond OpenAPI contract drift
4. first-party Ortyo usage evidence before expanding the verification DSL

Do not spend another sprint trying to prove absence for the four unresolved P0/P1 cells unless a product decision depends on them.
