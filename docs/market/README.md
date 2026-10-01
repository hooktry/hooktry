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
  -> relay or replay
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

Ortyo's stronger potential differentiation is the combination of:

- canonical evidence rather than log-only inspection
- portable Recording/Replay
- Contracts
- Scenario outcomes
- cardinality and ordering semantics
- explicit observation windows
- deterministic CI exit semantics
- correlation/idempotency context
- one model shared by HTTP, CLI, and MCP

That hypothesis still requires external demand evidence; competitor absence alone does not prove demand.

## Files

- `scope.yaml` - bounded market, cohort roles, products, stopping rule
- `vectors.yaml` - exploit/explore vectors and activation/stop triggers
- `capabilities.yaml` - normalized capability ontology and initial Ortyo overlay
- `depth-profiles.yaml` - observable dimensions for comparing capability depth
- `personas.yaml` - primary users and jobs-to-be-done
- `scenarios.yaml` - mechanism-independent end-to-end webhook outcomes
- `DEPTH_REPORT.md` - primary-cohort depth comparison and gap analysis
- `competitors.yaml` - product positioning and authoritative sources
- `observations/seed.yaml` - initial atomic market evidence

## Next passes

1. Complete atomic evidence for the primary cohort.
2. Add depth profiles for inspection, replay, local development, CI, agent operation, and reliable delivery.
3. Add persona/jobs and end-to-end scenarios.
4. Build a product-by-capability projection without an aggregate score.
5. Record pricing/retention/limits separately because these are volatile.
6. Add direct-demand evidence before converting market gaps into roadmap priority.
