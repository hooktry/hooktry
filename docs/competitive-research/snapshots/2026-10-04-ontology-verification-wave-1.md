# S004 - Ontology Verification, Wave 1

**Snapshot ID:** S004  
**Captured:** 2026-10-04  
**Selection universe:** S005 + S006  
**Subjects:** 19 products including Hooktry  
**Coverage rows:** 190 capability checks  
**Evidence records:** 19  
**Status:** Wave 1 complete - contract/implementation verification; executed E2E remains explicitly separate

## Why Wave 1 exists

S003 extracted an ontology from product descriptions. S004 tests that ontology against current product contracts and implementations instead of counting marketing words.

The original RFC asked for hands-on verification. This research environment cannot perform arbitrary external POST/authenticated browser actions against third-party SaaS, so S004 deliberately separates evidence levels instead of pretending documentation review is execution.

### Evidence levels

- **E1 Live surface** - current public product surface is directly observable.
- **E2 Contract verified** - first-party API/CLI/MCP/docs explicitly define the behavior.
- **E3 Implementation verified** - public or owned repository implementation/contracts are inspectable.
- **E4 Executed** - end-to-end behavior was actually run against the product.

Wave 1 contains:

- 2 E1 subjects
- 10 E2 subjects
- 7 E3 subjects
- 0 E4 subjects

The missing E4 status is recorded per product in Google Sheet tab `19 Ontology Verification`; it is not silently promoted.

## Subjects

1. Hooktry
2. Webhook.site
3. RequestBin.net
4. RelayFox
5. Splithook
6. PutsReq
7. Beeceptor
8. Request Catcher
9. HookSense
10. Webhook Toolkit
11. webhook.co
12. Exende Callback API
13. Hookdeck
14. WebhookX
15. HookTrace
16. smee.io
17. OpenWebhook
18. HookCapsule
19. HookTray

The sample intentionally spans classic bins, capture-first inboxes, signature-aware replay, response simulators, agent callback systems, reliability gateways, stateless relays, local-first products and scenario/fixture systems.

## Verification loop

The same ten primitives were checked for every subject:

`provision -> receive -> inspect -> persist -> replay -> forward -> verify/sign -> wait/subscribe -> respond -> cleanup`

Agent-operability and object model were captured separately.

## Aggregate results

| Primitive | Yes | Partial | No | Unknown |
| --- | ---: | ---: | ---: | ---: |
| Provision | 19 | 0 | 0 | 0 |
| Receive | 19 | 0 | 0 | 0 |
| Inspect | 19 | 0 | 0 | 0 |
| Persist | 15 | 3 | 1 | 0 |
| Replay | 13 | 0 | 4 | 2 |
| Forward | 14 | 2 | 3 | 0 |
| Verify / Sign | 7 | 3 | 3 | 6 |
| Wait / Subscribe | 6 | 6 | 7 | 0 |
| Respond | 5 | 3 | 8 | 3 |
| Cleanup | 10 | 1 | 1 | 7 |
| Agent-operable | 6 | 9 | 4 | 0 |

## Finding 1 - provision, receive and inspect are commodities

Every selected product exposes all three.

For Hooktry, none of these is a credible differentiator:

- create/get an endpoint;
- receive an HTTP request;
- show headers/body/path.

They are entry requirements.

The UX of these primitives can still differentiate - zero signup, speed, safety, naming, sharing, or programmatic creation - but their existence does not.

## Finding 2 - persist + replay + forward define the modern rich-debugger baseline

Persistence is explicit in 15/19 subjects, replay in 13/19, and forwarding in 14/19.

The important qualification is that persistence is not always better.

Four products intentionally expose a different durable boundary:

- Request Catcher - primarily live stream;
- smee.io - no server-side payload persistence;
- OpenWebhook - browser/MCP-process local history by default;
- HookTray - browser IndexedDB, server stateless by default.

This reinforces the S003/S006 durable-point axis.

## Finding 3 - signature-aware replay is still strategically meaningful

Only seven subjects have clearly verified first-class sign/verify behavior in Wave 1:

- Splithook
- HookSense
- Webhook Toolkit
- webhook.co
- Hookdeck
- WebhookX
- HookTrace

Three more are partial/scriptable rather than explicit.

The strongest example remains Splithook: preserve raw body, refresh timestamp and re-sign a delayed replay so production verification code remains enabled.

For Hooktry, provider-aware signature verification/re-signing is not yet as explicit in the reviewed product contract as Recording/Contract/Scenario semantics.

This is a real gap candidate, not commodity parity.

## Finding 4 - wait is not common enough to be table stakes

Only six subjects have a clear first-class `Yes` for Wait/Subscribe and also a `Yes` for Agent-operable:

- Hooktry
- HookSense
- Webhook Toolkit
- webhook.co
- Exende Callback API
- OpenWebhook

However these six implement materially different meanings.

### HookSense

`create_callback_endpoint -> wait_for_callback(after cursor)`

The endpoint is a hosted callback primitive and the agent is the intended actor.

### Webhook Toolkit

`createEndpoint -> trigger application -> waitForRequest(filter)`

The endpoint behaves like a test fixture and integrates directly with Vitest/Jest/Playwright.

### webhook.co

`triggers.create -> triggers.wait(cursor)`

The durable event exists independently of the agent and the cursor provides at-least-once acknowledgment semantics.

### Exende

`create callback -> wait(after) -> delete`

Minimal temporary resource semantics: 10-minute lifetime, private read token, explicit delete.

### OpenWebhook

`url.inspect -> watch.start -> watch.wait`

The wait is local/in-memory around a stateless relay rather than a server-side durable inbox.

### Hooktry

Scenario completion uses persisted Interaction evidence plus non-polling arrival notification and an observation/settle window. It is already synchronization-aware, but its public product language is currently more Scenario-oriented than a simple generic `wait_for_interaction` primitive.

### Implication

The opportunity is not "invent wait".

S006 proved wait/stream semantics existed in WebhookInbox in 2013.

The opportunity is:

> bind waiting to canonical evidence, trust, matching, assertions and lifecycle cleanup for autonomous software.

## Finding 5 - response control remains surprisingly rare

Only five subjects have clearly verified first-class response control:

- Webhook.site
- PutsReq
- Beeceptor
- Webhook Toolkit
- OpenWebhook

Response control matters when testing the **sender**, not only the receiver.

Examples:

- return 500 and observe provider retry;
- return 429;
- delay response;
- return provider-specific challenge;
- return dynamic body/headers;
- proxy localhost's real response.

This strongly supports making response policy part of Hooktry's Fixture/Scenario model rather than treating it as dashboard polish.

## Finding 6 - object model predicts product category better than feature count

The selected subjects cluster around different durable objects.

### Request-bin family

`Endpoint / Token -> Request`

Examples:
- Webhook.site
- RequestBin.net
- RelayFox
- PutsReq

### Agent callback family

`Endpoint / Callback -> Wait cursor -> Callback/Event`

Examples:
- HookSense
- Exende
- OpenWebhook

### Reliability family

`Source / Route -> Event -> Delivery -> Attempt -> Destination`

Examples:
- Hookdeck
- WebhookX
- HookTrace
- webhook.co

### Test-evidence family

`Interaction/Event -> Recording/Capsule -> Scenario/Contract -> Outcome/Test`

Examples:
- Hooktry
- HookCapsule
- Webhook Toolkit, in a lighter-weight form

### Stateless/local-first family

`Channel/Session -> live request snapshot -> client-local state`

Examples:
- smee.io
- HookTray
- OpenWebhook free inspector

This supports the S003 conclusion that a one-dimensional "request bin -> gateway" maturity ladder is misleading.

## Finding 7 - HookCapsule validates Scenario as a real job, not only our abstraction

HookCapsule converts:

`real failure -> redact -> ordered Capsule -> replay -> executable Vitest regression fixture`

This is independently convergent evidence for Hooktry's:

- Recording
- Scenario
- ordering
- replay
- assertions
- persisted Outcome

model.

The strongest difference is that HookCapsule starts from an incident tape and exports a regression fixture, while Hooktry treats the integration boundary and canonical Interaction as a more general programmable primitive.

## Finding 8 - Hooktry's strongest current differentiation is the composition

The reviewed Hooktry repository already exposes:

- Boundary
- Session
- Interaction
- Recording
- Contract
- Scenario
- ScenarioOutcome
- deterministic cardinality
- correlation context
- observed ordering
- non-polling observation window
- replay
- automatic Exposure revoke
- CLI/MCP/HTTP parity

Individual primitives are not unique.

The composition is unusual.

A more accurate competitive statement after S004 Wave 1 is:

> Hooktry is not differentiated because it can receive or replay a webhook. It is differentiated if it can turn an external interaction into canonical evidence that a human, CI job, or agent can deterministically replay, match, assert and dispose of.

## Current Hooktry gaps / verification targets

### G1 - provider trust semantics

Verify whether provider-specific:

- signature verification
- fresh signature generation
- replay re-signing
- timestamp tolerance diagnostics

belong in Hooktry core, provider adapters, or test helpers.

Splithook, Webhook Toolkit and webhook.co set a strong benchmark here.

### G2 - generic wait primitive

Hooktry has non-polling Scenario observation, but S004 should test whether a simpler:

`wait_for_interaction(boundary, matcher, after, timeout)`

belongs in the API/MCP alongside Scenario.

This could make simple agent callbacks much lighter without weakening the richer Scenario model.

### G3 - response policy

Test a first-class response policy attached to Boundary/Fixture/Scenario:

- status
- headers
- body
- delay
- timeout/disconnect
- dynamic rule

The sender-testing value is well established historically and currently.

### G4 - terminology

The sample supports keeping:

`Interaction != Event != Delivery != Attempt`

Hooktry's `Interaction` remains useful because it can retain protocol truth without forcing all observations into production event-delivery semantics.

### G5 - gateway boundary

Retry/DLQ/delivery-attempt infrastructure is strong in Hookdeck, WebhookX, HookTrace and webhook.co.

Hooktry should not absorb those semantics accidentally.

A clean boundary remains:

- Hooktry: integration evidence, testing, control and proof;
- gateway products: production event delivery reliability.

## Table-stakes vs differentiators after Wave 1

### Commodity

- Provision
- Receive
- Inspect

### Expected in a rich development product

- Persist, with an explicit privacy/durable-point choice
- Replay
- Forward/local delivery
- machine-readable API/CLI surface

### Differentiating or still sparse

- signature-aware replay
- first-class verification diagnostics
- wait/resume semantics
- deterministic match/filter while waiting
- response policy
- Scenario/Fixture abstractions
- assertions and persisted proof
- explicit cleanup/claim lifecycle
- local-first privacy with machine control

### Neighboring infrastructure, not automatic Hooktry scope

- production retry queues
- DLQ
- delivery-attempt scheduling
- multi-tenant production gateway controls
- large-scale fan-out

## Executed follow-up contract

Wave 1 records an explicit next executed step for every product in `19 Ontology Verification`.

The executed phase should only upgrade evidence to E4 after the actual external behavior is observed.

Examples:

- Webhook.site: anonymous create -> POST -> retrieve -> response modification -> delete
- RequestBin.net: create -> send -> list -> replay -> delete
- Splithook: signed Stripe capture -> delayed re-sign/replay
- HookSense: create -> external callback -> wait -> verify -> replay
- Webhook Toolkit: createEndpoint -> trigger -> waitForRequest(filter) -> verify -> replay
- webhook.co: verified event -> cursor-acked agent trigger -> failed delivery -> retry/DLQ
- Exende: x402 create -> POST -> wait(after) -> read -> delete
- HookCapsule: reversed event order -> replay failure -> corrected order -> exported test
- Hooktry: hosted Scenario -> external interaction -> canonical evidence -> replay -> assertions -> automatic cleanup

## Workbook outputs

- `19 Ontology Verification` - one-row-per-product verification matrix
- `03 Feature Coverage` - 190 product × primitive checks
- `05 Workflow Benchmarks` - one canonical workflow record per subject
- `11 Evidence` - 19 S004 evidence records

S004 Wave 1 is frozen at these evidence levels. Later executed verification should not rewrite what Wave 1 claimed; it should add higher-grade evidence.
