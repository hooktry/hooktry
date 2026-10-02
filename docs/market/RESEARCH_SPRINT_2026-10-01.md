# Hooktry MCIF research sprint 1

Checked: 2026-10-01  
DWC: MCIF/Hooktry.9 RESEARCH

## Goal

Execute the first decision-value research sprint produced by `market-research-debt`.

The sprint starts with the top P0/P1 competitor-capability checks and changes a matrix cell only when current official evidence supports a concrete state.

## Scope

Top P0/P1 checks researched:

- Hookdeck / search-filter
- Webhook Relay / search-filter
- Beeceptor / search-filter
- Hookdeck / custom-response
- Webhook Relay / custom-response
- Hook0 / custom-response
- Hookdeck / failure-simulation
- Hooklistener / failure-simulation
- Webhook Relay / failure-simulation
- Hookdeck / provider-templates
- Hooklistener / provider-templates
- Webhook Relay / provider-templates
- Hookdeck / signature-verification
- Hooklistener / signature-verification
- Webhook Relay / signature-verification

## Results

11 of 15 checks were resolved with current official evidence.

Resolved:

- Hookdeck / search-filter -> `present`
- Webhook Relay / search-filter -> `partial`
- Beeceptor / search-filter -> `present`
- Hookdeck / custom-response -> `partial`
- Webhook Relay / custom-response -> `present`
- Hooklistener / failure-simulation -> `present`
- Webhook Relay / failure-simulation -> `partial`
- Hookdeck / provider-templates -> `present`
- Hookdeck / signature-verification -> `present`
- Hooklistener / signature-verification -> `present`
- Webhook Relay / signature-verification -> `partial`

Still unknown:

- Hook0 / custom-response
- Hookdeck / failure-simulation
- Hooklistener / provider-templates
- Webhook Relay / provider-templates

The unresolved cells remain `unknown`. Failure to find current official evidence is not evidence of absence.

## Evidence interpretation

### Search/filter

Hookdeck and Beeceptor both expose meaningful payload/history search. Webhook Relay exposes structured log filters for status, destination/output, and time range, but the current official evidence does not establish equivalent payload-content search.

Result: Hooktry's `search-filter` remains a `must` and remains an implementation-depth gap because Hooktry is still `partial`.

### Custom sender response

Webhook Relay exposes static status/body/header responses and dynamic output-driven responses. Hookdeck exposes customizable synchronous response bodies and provider-aware response behavior, but the researched docs do not establish the same full generic status/header/delay surface.

Result: `custom-response` remains `must`; Hooktry's partial implementation still deserves current-horizon work.

### Failure simulation

Hooklistener can explicitly return failure status codes with response delay. Webhook Relay can force controlled error responses but the researched docs do not establish a general latency/fault profile.

No current official Hookdeck evidence established a comparable failure-simulation surface.

Result: `failure-simulation` remains `should` in the current horizon. The market evidence supports controlled failure testing, but depth varies materially.

### Provider-aware testing

Hookdeck has a broad provider Source Type catalog with provider-aware authentication, response behavior, guides, skills, and sample payloads.

The sprint did not establish equivalent provider-template generation in Hooklistener or Webhook Relay.

Result: `provider-templates` remains `should` / next. The evidence strengthens the cross-provider direction, but direct demand is still mixed because provider-native CLIs remain substitutes.

### Signature verification

Hookdeck supports provider-aware and generic HMAC verification. Hooklistener exposes Stripe/GitHub/Slack verification through its MCP surface. Webhook Relay supports programmable HMAC verification and provider walkthroughs, but not the same turnkey provider-template depth in the evidence reviewed.

Result: `signature-verification` remains `should` / next and is now more strongly established as a competitive capability.

## Decision impact

No disposition changed in this sprint.

No Hooktry implementation status changed in this sprint.

No new market-motion signal was recorded.

The changes are **research resolution**:

- external evidenced cells: 59 -> 70
- external unknown cells: 171 -> 160
- atomic observations: 69 -> 80
- matrix research resolutions: 11
- observed market-state changes: 0
- Hooktry motion: 0

This is exactly the distinction enforced by `market-trend`: learning what a competitor already supports is not proof that the competitor just changed.

## Next research slice

Continue only where the remaining evidence can still change decisions:

1. finish unresolved P0 cells, especially Hookdeck failure simulation and Hook0 custom response
2. finish provider-template uncertainty for Hooklistener and Webhook Relay
3. move to P2 deterministic-CI depth
4. inspect request-diff depth for Hookdeck and Webhook Relay

Stop researching a capability once additional unknown cells are unlikely to change its disposition, implementation depth, differentiation thesis, or vector activation decision.
