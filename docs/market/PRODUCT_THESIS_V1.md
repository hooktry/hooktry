# Hooktry Product Thesis v1

**Status:** active product thesis  
**Date:** 2026-10-04  
**Evidence base:** S003 ontology, S004 verification, S006 category archaeology, S007 E4 execution  
**Primary wedge:** webhook and external-callback development, testing, and pilot

## One-sentence thesis

> **Hooktry is a programmable public interaction boundary that turns external callbacks into canonical, reproducible proof for humans, CI, and software agents.**

The product should make it unusually cheap to go from:

```text
external system
    -> public Hook
    -> Interaction
    -> inspect / wait / respond
    -> replay
    -> assert
    -> persisted Outcome
    -> cleanup or claim
```

The wedge is webhooks. The deeper product is **integration evidence and control**.

## The user job

A developer is integrating with a system they do not fully control.

They need to:

1. give that system a reachable URL;
2. observe exactly what it sent;
3. control what the sender sees in response;
4. reproduce the interaction without repeatedly triggering the provider;
5. wait for the correct interaction in automation;
6. verify the integration deterministically;
7. keep a machine-readable proof of what happened;
8. dispose of the temporary boundary or claim it when it becomes valuable.

This job is performed by three first-class actors:

- human developer;
- CI/test process;
- software agent.

The actor may change. The evidence model must not.

## What Hooktry is

Hooktry is a **programmable interaction boundary + evidence system**.

Its public webhook-specific primitive is the **Hook**, backed by the broader **Exposure/Boundary** domain model.

Its durable product value is not the URL itself. It is the continuity of evidence around the URL.

```text
Hook / Boundary
    |
    v
Interaction
    |
    +--> inspect
    +--> correlate
    +--> wait / match
    +--> respond
    |
    v
Recording
    |
    v
Replay
    |
    v
Contract / Scenario
    |
    v
Outcome / proof
```

A Hook can be disposable or claimable.

An Interaction is canonical protocol evidence.

A Recording makes observed reality reusable.

A Contract describes expected behavior.

A Scenario binds temporal expectations and evidence into a reproducible verification run.

An Outcome is persisted proof.

## What Hooktry is not

Hooktry is not primarily:

- a generic reverse tunnel;
- a production webhook delivery bus;
- a retry/DLQ/fan-out platform;
- a general API gateway;
- a workflow automation platform;
- a generic mock-server suite;
- a managed devbox/compute product;
- an AI-agent platform;
- a collection of provider-specific webhook toys.

These products may be neighbors, substrates, or integrations.

Hooktry should only absorb their capabilities when they strengthen the same interaction-evidence lifecycle.

## The category distinction

### Commodity

S004/S007 show that these are table stakes:

- provision a public endpoint;
- receive HTTP;
- inspect headers/body/path;
- retain some request history.

They matter enormously for UX, but their existence is not differentiation.

### Rich development baseline

A serious modern product increasingly needs:

- replay;
- local forwarding or another development destination;
- machine-readable API/CLI surface;
- an explicit durability/privacy model.

### Hooktry differentiation

Hooktry should differentiate through the composition:

```text
canonical evidence
    +
machine-safe synchronization
    +
controllable receiver behavior
    +
deterministic replay
    +
contracts / temporal assertions
    +
persisted proof
    +
explicit lifecycle
```

No single item above is unique.

The product claim is that the **same Interaction evidence survives the whole loop**.

## The invariant

Every core feature must strengthen at least one of:

1. **Reachability** - let an external system reach the boundary.
2. **Evidence fidelity** - preserve what actually happened.
3. **Control** - determine what the sender or downstream receiver observes.
4. **Reproducibility** - replay or reconstruct behavior deterministically.
5. **Verification** - match, assert, order, count, correlate, or prove.
6. **Machine operability** - make the same lifecycle usable by CI and agents.
7. **Lifecycle safety** - bound, expire, claim, revoke, or clean up authority.

If a capability does not strengthen this loop, it is probably an integration, substrate, adjacent product, or out of scope.

## Core product loop

### Fast path

The simple path must remain excellent forever:

```text
open Hooktry
    -> Hook already exists or create one
    -> copy URL
    -> send webhook
    -> see Interaction immediately
```

Do not force users into projects, workflows, scenarios, or authentication before value exists.

This is a historical lesson from RequestBin and Webhook.site.

### Programmable path

```text
create Hook
    -> receive or wait for matching Interaction
    -> inspect canonical evidence
    -> optionally control response
    -> replay / forward
    -> assert Contract or complete Scenario
    -> read persisted Outcome
    -> delete / expire / revoke / claim
```

HTTP, CLI, MCP, test libraries, and UI should be projections over the same lifecycle.

## Actor model

### Human

Optimizes for:

- instant URL;
- realtime inspection;
- readable evidence;
- one-click replay;
- understandable diffs;
- low ceremony.

### CI

Optimizes for:

- deterministic setup;
- bounded wait;
- exact matching;
- stable exit semantics;
- cleanup;
- machine-readable Outcome.

### Agent

Optimizes for:

- programmatic provisioning;
- typed capabilities;
- no browser dependency;
- non-wasteful wait/resume;
- canonical evidence;
- explicit authority;
- safe cleanup;
- proof that can be cited by the next action.

Agent support is therefore **an interface/property of the product**, not the entire category definition.

## P1 - make the integration boundary complete

P1 means capabilities that should define the near-term product, not every implementation detail that ships in one release.

### P1.1 Instant anonymous Hook

Preserve the anonymous-first loop:

- instant provision;
- separate hook/view/claim authority;
- bounded TTL/quota;
- realtime + durable evidence;
- claim without changing the public URL;
- explicit revoke/delete.

This is acquisition and product utility, not a demo mode.

### P1.2 Canonical Interaction inspection

Interaction remains the source of truth for:

- method/path/query;
- raw body;
- normalized body where safe;
- headers;
- timing;
- source/correlation context;
- observed response;
- origin;
- lineage to replay/source Interaction.

Do not invent separate evidence models for UI, CLI, MCP, and Scenario.

### P1.3 Replay + development forwarding

Replay must be deterministic enough to support debugging and verification.

Required near-term semantics:

- exact replay of captured evidence;
- replay lineage;
- replay to local/staging/public destination;
- explicit mutation when the user chooses to edit;
- clear distinction between original and replayed Interaction.

### P1.4 Generic `wait_for_interaction`

Add a small synchronization primitive alongside Scenario:

```text
wait_for_interaction(
  hook_or_boundary,
  matcher,
  after,
  timeout
) -> Interaction
```

Requirements:

- checks already-arrived evidence first;
- no blind polling loop as the canonical implementation;
- bounded timeout;
- matcher/filter;
- cursor or `after` semantics;
- returns the canonical Interaction, not a second callback object;
- usable from API/CLI/MCP/test library.

This is not a replacement for Scenario.

It is the smallest useful agent/CI callback primitive.

### P1.5 Response Policy v1

Make receiver behavior first-class.

Initial policy:

- status;
- headers;
- body;
- content type;
- delay;
- bounded failure simulation.

The policy belongs to the same Hook/Boundary/Fixture lifecycle and must be observable in Interaction evidence.

Do not start with a general-purpose scripting engine.

### P1.6 Contracts + Scenario + persisted Outcome

Preserve and productize the already-executed model:

- request/response Contract;
- exact/ranged cardinality where justified;
- correlation/idempotency context;
- ordering;
- observation horizon;
- settle window;
- deterministic replay;
- assertion results;
- persisted ScenarioOutcome;
- idempotent completion;
- automatic cleanup/revoke where appropriate.

The UI should make the progression from a real Interaction to reusable proof understandable.

### P1.7 Surface parity

Core lifecycle semantics should exist consistently across:

- Web UI;
- HTTP API;
- CLI;
- MCP;
- first-party test library where useful.

Do not build an MCP-only feature and call it agent-native.

## P2 - increase fidelity and reuse

### P2.1 Provider trust adapters

Cross-provider adapters for:

- signature verification;
- timestamp tolerance diagnostics;
- signing;
- replay re-signing;
- provider-specific canonical raw-body handling.

Start with a small set of providers based on usage.

The abstraction should remain cross-provider.

### P2.2 Scenario / fixture derivation from captured reality

Reduce the distance from:

`this failed in reality`

to:

`this is now a reproducible test`.

Candidate operations:

- save Interaction(s) as Recording;
- derive Contract;
- derive Scenario;
- redact secrets;
- preserve order/correlation;
- export or invoke as regression test.

HookCapsule E4 is strong independent evidence for this job.

### P2.3 Advanced response behavior

Only after Response Policy v1 proves useful:

- conditional rules;
- timeout/disconnect;
- sequences/state transitions;
- dynamic templates;
- challenge flows.

Do not silently become a general mock platform.

### P2.4 Better local/private reachability

Deepen local forwarding only where it preserves Hooktry's evidence advantage:

```text
provider
   -> durable Hooktry capture
   -> local destination
```

The laptop being offline should not erase canonical evidence.

Pure tunnel functionality remains substitutable.

### P2.5 Portable evidence bundles

A portable artifact may include:

- selected canonical Interactions;
- redaction metadata;
- Contracts;
- Scenario;
- Outcome;
- lineage/provenance;
- replay metadata.

The artifact should make a bug or integration proof transferable across developer, CI, agent, and repository.

## Not now

### Production delivery infrastructure

Do not make these core merely because webhook platforms have them:

- durable retry scheduler;
- DLQ;
- tenant-scale fan-out;
- production delivery attempt orchestration;
- production destination health management.

Hookdeck, WebhookX, HookTrace, webhook.co and similar products already occupy this territory.

Re-evaluate only if Hooktry Hooks naturally graduate into long-lived production endpoints and users explicitly demand continuity.

### General workflow automation

Do not build a Pipedream/Zapier/n8n competitor.

Hooktry can emit evidence into those systems or receive events from them.

### Full API virtualization

Response Policy and provider emulation can grow from the same boundary model.

Do not build an unrelated generic mock-server product unless webhook users repeatedly need it and the canonical Interaction/Scenario model clearly transfers.

### Generic tunneling

Do not compete with ngrok/Cloudflare Tunnel/Tailscale on reachability breadth.

Use, integrate, or implement only the thin reachability required for the evidence loop.

### Multi-protocol expansion

SMTP, queues, gRPC, WebSockets, object storage, etc. remain future vectors.

Promote a protocol only when the same evidence lifecycle transfers cleanly and users ask for it.

### Managed compute

VM/container/devbox infrastructure remains replaceable substrate.

### Agent platform expansion

Approvals and safe execution may remain valuable control surfaces, but Hooktry should not drift into generic agent orchestration.

## Explicit non-differentiators

Do not position Hooktry around any of these alone:

- "we give you a webhook URL";
- realtime request inspection;
- replay;
- MCP;
- AI;
- self-hosting;
- a CLI;
- a pretty payload viewer.

All are useful.

None is a sufficient product thesis.

## Positioning hypothesis

### Internal category

**Programmable Integration Evidence Boundary**

This is an internal category label, not necessarily landing-page copy.

### External plain-language description

> **Hooktry gives developers, CI, and agents a public integration endpoint, then turns every real callback into evidence they can wait for, replay, assert, and keep as proof.**

### Short promise

> **From callback to proof.**

Keep this provisional until messaging tests demonstrate that users understand it.

## Why this is credible now

### S006 historical evidence

History shows:

- reachability and evidence are separate roots;
- wait/programmatic response are old primitives;
- anonymous receivers repeatedly face abuse/lifecycle pressure;
- simple request-bin UX remains valuable even when products grow.

Therefore Hooktry should not claim novelty from a URL, wait, or API alone.

### S007 execution evidence

E4 execution proved:

- Webhook.site's anonymous receiver + response control works;
- Webhook Toolkit's small `create -> wait(filter) -> replay -> cleanup` fixture loop works;
- signing/verification is usable as a developer primitive;
- smee.io's live relay works;
- OpenWebhook's public-to-local response passthrough works;
- HookTray's stateless boundary is a legitimate trust model;
- HookCapsule's incident -> ordered capsule -> replay -> regression artifact works;
- Hooktry's MCP and CLI Scenario flows execute real HTTP and produce replayed/asserted persisted proof.

The remaining opportunity is the composition.

## Product decision test

For every proposed roadmap item, answer:

1. What primary integration-development job does it improve?
2. Which canonical object does it attach to?
3. What evidence does it create or preserve?
4. Can human, CI, and agent use the same semantics?
5. Does it strengthen the fast path or the proof path?
6. What dedicated product already does this better?
7. Why should Hooktry own it rather than integrate with it?
8. What is the smallest slice that tests the hypothesis?

If these cannot be answered, do not promote the feature into core.

## v1 success signal

The thesis is working when a user can take one real asynchronous integration and complete this loop without conceptual jumps:

```text
get URL
 -> trigger provider
 -> wait/see the right interaction
 -> diagnose
 -> reproduce it
 -> change receiver behavior
 -> verify the fix
 -> persist proof
 -> clean up
```

And the exact same evidence can be consumed by a human, a CI job, or an agent.

That is the product.
