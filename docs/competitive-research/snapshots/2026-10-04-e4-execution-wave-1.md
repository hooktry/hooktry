# S007 - E4 execution wave 1

**Snapshot ID:** S007  
**Captured:** 2026-10-04  
**Parent verification:** S004 Wave 1  
**Subjects upgraded to E4:** 7  
**Feature checks upgraded to E4:** 38  
**Status:** frozen executed-evidence snapshot

## Purpose

Upgrade selected S004 claims from current-surface / contract / implementation evidence to actual executed behavior.

E4 means the behavior was run and observed. It does not mean every capability of that product was executed.

## Executed subjects

| Product | Execution mode | Result | Evidence |
| --- | --- | --- | --- |
| Hooktry | self-hosted local MCP + CLI integration | PASS | https://github.com/hooktry/hooktry/actions/runs/37211434289 |
| Webhook.site | public anonymous SaaS | PASS | https://github.com/hooktry/hooktry/actions/runs/37210725076 |
| Webhook Toolkit | public anonymous SaaS + local signature primitive | PASS | https://github.com/hooktry/hooktry/actions/runs/37210874736 and https://github.com/hooktry/hooktry/actions/runs/37211658107 |
| smee.io | public channel + local client | PASS | https://github.com/hooktry/hooktry/actions/runs/37210874736 |
| OpenWebhook | public anonymous CLI + localhost receiver | PASS | https://github.com/hooktry/hooktry/actions/runs/37210725076 |
| HookCapsule | self-hosted Docker + documented Playwright critical flow | PASS | https://github.com/hooktry/hooktry/actions/runs/37211040255 |
| HookTray | self-hosted API + SSE | PASS | https://github.com/hooktry/hooktry/actions/runs/37211040255 |

Raw `E4_RESULT` markers remain in the GitHub Actions job logs.

## What was actually executed

### Hooktry

Two independent flows passed.

MCP Scenario:

`scenario_create -> scenario_start -> real HTTP -> persisted Interaction -> Recording -> replay -> Contract assertion -> persisted Outcome -> repeat complete -> automatic Exposure revoke`

CLI Scenario:

`manifest -> child process -> real HTTP traffic -> response capture -> Contract assertion -> ScenarioOutcome -> usage proof`

This validates the current evidence/scenario composition at runtime rather than only through type/API inspection.

The first benchmark attempt failed before product behavior because the repository build script requires `apps/web/dist`. After building the shared UI prerequisite, both scenario flows passed. Treat this as repository DX/build-order evidence, not a behavioral product failure.

### Webhook.site

Executed anonymously:

`create token -> POST JSON -> retrieve persisted request -> change default response -> POST again -> observe 503/body -> delete token`

Observed sender responses:

- first request: HTTP 202
- second request after response update: HTTP 503

This confirms that response control is not only an advanced automation concept; it works in the simple anonymous receiver lifecycle.

### Webhook Toolkit

Executed anonymously:

`createEndpoint -> set 207 response -> POST -> waitForRequest(filter) -> getRequest -> replayRequest to Webhook.site -> confirm replay landed -> clearRequests -> deleteEndpoint`

A separate local execution proved:

`GitHub sign -> verify(valid)=true -> mutate body -> verify=false`

The initial public attempt timed out connecting to `webhook-toolkit.com` from one GitHub runner. A retry using IPv4-first networking succeeded. The final E4 claim is based only on the successful run.

This is currently the strongest directly executed benchmark for the small agent/test-fixture loop.

### smee.io

Executed:

`GET /new -> generated channel -> connect smee-client -> public POST -> local receiver observes payload`

This proves live relay semantics.

S007 does **not** upgrade the "no server-side persistence" claim to E4 because the executed run did not perform a send-while-disconnected / reconnect experiment. That remains E3 implementation evidence.

### OpenWebhook

Executed without an account:

`openwebhook listen -> temporary public URL -> public POST -> localhost receiver -> local HTTP 209/body -> same response observed by public sender`

This confirms a useful topology:

`public endpoint -> pull/live relay -> localhost -> response passthrough`

The MCP `watch.start/watch.wait` path was not executed in this wave and remains contract-level evidence.

### HookCapsule

Ran its documented Playwright critical flow against the self-hosted Docker stack:

`create inbox -> send payment.failed -> send payment.retry -> live capture -> select events -> build ordered Capsule -> replay to stateful target -> successful outcome -> export hookcapsule.zip`

This is strong independent execution evidence that "real incident -> ordered reusable scenario -> replay -> executable regression artifact" is a real product job.

### HookTray

Executed its stateless boundary directly:

1. create hook token;
2. POST before any SSE subscriber;
3. response says `delivered=false`;
4. connect SSE stream;
5. POST second event;
6. second event arrives live;
7. first event does not appear after subscription.

This is behavioral proof that a product may deliberately choose a live/stateless server boundary instead of hosted request persistence.

## Feature-level evidence discipline

Only primitives actually exercised were upgraded to E4.

Examples:

- Webhook Toolkit's wait/replay/respond/sign/verify/cleanup are E4.
- smee.io's provision/receive/forward are E4, but server non-persistence remains E3.
- OpenWebhook's provision/receive/local-forward/response-passthrough are E4, while MCP wait remains E2.
- Hooktry's Scenario/evidence/replay/assertion/cleanup composition is E4, while provider-specific signing remains below E4.

The Google Sheet reflects this in `03 Feature Coverage` rather than upgrading an entire product indiscriminately.

## Strongest product conclusions after execution

### 1. Webhook Toolkit is a real direct benchmark, not marketing vapor

Its anonymous public test loop works in practice and is small:

`create -> trigger -> filtered wait -> inspect -> replay -> cleanup`

It additionally has executed sign/verify semantics.

Hooktry should treat this as a concrete direct competitor for agent/CI integration testing.

### 2. Hooktry's strongest territory remains proof, not capture

The executed Hooktry flow demonstrates a richer terminal artifact:

`Interaction -> Recording -> replay -> Contract assertion -> ScenarioOutcome`

This is meaningfully different from a request list even when competitors share provision/capture/replay.

### 3. HookCapsule validates the Scenario hypothesis independently

HookCapsule's ordered Capsule and exported regression test are an independent convergence toward the same higher-order job.

The competitive question is therefore not whether reusable scenario/evidence is needed, but which abstraction and workflow wins.

### 4. Response behavior deserves first-class design

Webhook.site and Webhook Toolkit both executed receiver response control successfully.

OpenWebhook executed response passthrough from localhost.

The ability to control what the sender observes is operationally central to testing retries, error handling and synchronous callbacks.

### 5. Durability must be explicit

Execution now confirms at least three different legitimate models:

- hosted durable evidence - Webhook.site / Webhook Toolkit;
- live relay - smee.io;
- stateless server + client-side history - HookTray / OpenWebhook model.

"Persist" is not simply a yes/no maturity score. It is a trust and topology decision.

## Immediate Hooktry research/product implications

### Candidate P1 - generic wait primitive

Keep Scenario as the richer proof model, but evaluate a lightweight:

`wait_for_interaction(boundary, matcher, after, timeout)`

Webhook Toolkit proves the ergonomic value of a filtered wait in a real anonymous test loop.

### Candidate P1 - response policy

Evaluate first-class response policy attached to Boundary/Fixture/Scenario:

- status
- headers
- body
- delay
- timeout/disconnect
- rule-based response

This is now both historically and execution validated.

### Candidate P1/P2 - signature adapters and replay re-signing

Webhook Toolkit proves generic signing/verification works as a usable developer primitive.

Splithook still sets the contract-level benchmark for provider-aware delayed replay with fresh signatures.

Hooktry should decide whether provider trust belongs in core, adapters, or a test helper package.

### Preserve - Scenario/Outcome architecture

Do not collapse Scenario into a simple request-wait API.

Executed Hooktry and HookCapsule flows both support the value of a higher-order reproducible evidence object.

### Avoid - accidental gateway expansion

Nothing in E4 changes the boundary established by Hookdeck/WebhookX/HookTrace:

production retries, DLQ and delivery scheduling remain a neighboring infrastructure category rather than automatic Hooktry scope.

## Workbook changes

S007 upgrades:

- 7 rows in `19 Ontology Verification` to `E4 Executed`;
- 38 specific capability rows in `03 Feature Coverage` to E4;
- 7 rows in `05 Workflow Benchmarks` to Executed=Yes;
- 7 new E4 records in `11 Evidence`.

S004 Wave 1 remains frozen and should not be rewritten.
