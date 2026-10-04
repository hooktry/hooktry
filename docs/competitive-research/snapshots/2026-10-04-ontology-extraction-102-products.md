# S003 - Ontology extraction from the 102-product corpus

**Snapshot ID:** S003  
**Captured:** 2026-10-04  
**Source corpus:** S002 / Market Map rows for 102 products including Hooktry  
**Related discovery passes:** P00-P12  
**Status:** frozen analytical snapshot - non-canonical

## Purpose

Extract a market ontology from the 102-product corpus without yet doing deep feature verification for every product.

This snapshot asks:

- Which objects recur across webhook/request/event products?
- Which actions recur?
- Which concepts belong mainly to direct debugging tools versus adjacent reliability infrastructure?
- Which concepts are genuinely emerging around autonomous agents?
- Which abstractions appear useful for Hooktry even when the market does not commonly name them?

## Method

For each product, the analysis used the normalized Market Map fields:

- Category
- Primary Job
- Distinctive Angle
- Notes

A concept counts at most once per product. Repeated words inside one product do not increase its count.

These are **lexical product-evidence counts**, not verified feature-coverage counts. A low count can mean a capability is under-described in our current summaries rather than truly rare.

The normalized results are materialized in Google Sheet tab `02 Feature Taxonomy`.

## Most common actions

| Action | Product evidence | Direct | Adjacent | Substitute |
| --- | ---: | ---: | ---: | ---: |
| Inspect / observe | 80 | 58 | 18 | 4 |
| Receive / capture | 67 | 52 | 11 | 4 |
| Replay / redeliver | 53 | 35 | 15 | 2 |
| Programmability - API/SDK/CLI/MCP | 51 | 34 | 11 | 6 |
| Forward / route / fan-out | 48 | 29 | 15 | 4 |
| Persist / store | 41 | 23 | 15 | 3 |
| Provision endpoint | 38 | 33 | 1 | 4 |
| Verify / sign / re-sign | 33 | 16 | 14 | 2 |
| Expose / tunnel to localhost | 30 | 20 | 2 | 8 |
| Retry / backoff / DLQ | 27 | 6 | 21 | 0 |
| Simulate / mock | 21 | 12 | 5 | 3 |
| Wait / subscribe / resume | 14 | 9 | 4 | 1 |
| Expire / claim / dispose | 14 | 14 | 0 | 0 |
| Monitor / alert | 13 | 9 | 4 | 0 |
| Respond / control response | 12 | 9 | 1 | 2 |
| Transform | 9 | 4 | 5 | 0 |
| Share / collaborate | 8 | 6 | 2 | 0 |
| Deduplicate / idempotency | 2 | 0 | 2 | 0 |
| Assert programmatically | 2 | 2 | 0 | 0 |

### Directness asymmetry

Two splits are especially important.

**Lifecycle is a development-tool concern.** Endpoint provision is 33/38 direct and lifecycle/expiry evidence is 14/14 direct.

**Reliability is a neighboring infrastructure concern.** Retry/backoff/DLQ is 21/27 adjacent. Delivery attempts are 5/5 adjacent.

This is evidence that production reliability is not merely another checkbox on a request bin. It is a neighboring ontology layer with different durable objects and failure semantics.

## Most visible objects

| Object | Product evidence | Direct | Adjacent | Substitute |
| --- | ---: | ---: | ---: | ---: |
| Request | 40 | 33 | 4 | 3 |
| Event | 27 | 12 | 9 | 6 |
| Endpoint | 22 | 19 | 0 | 3 |
| Delivery | 22 | 6 | 13 | 3 |
| Agent | 18 | 11 | 6 | 0 |
| Payload | 14 | 10 | 2 | 2 |
| Response | 14 | 9 | 2 | 3 |
| History | 14 | 10 | 3 | 1 |
| Relay | 13 | 9 | 3 | 1 |
| Workflow | 12 | 6 | 2 | 4 |
| Tunnel | 11 | 5 | 0 | 6 |
| Inbox | 10 | 9 | 1 | 0 |
| Signature | 10 | 5 | 4 | 1 |
| Route | 7 | 3 | 4 | 0 |
| Destination | 6 | 2 | 4 | 0 |
| Attempt | 5 | 0 | 5 | 0 |
| Queue | 5 | 1 | 4 | 0 |
| Callback | 4 | 1 | 1 | 2 |
| Scenario | 3 | 0 | 1 | 1 |
| Fixture | 2 | 2 | 0 | 0 |

## The object transition

The corpus suggests a natural transition:

```text
HTTP debugger:
Endpoint -> Request -> Response

event receiver:
Endpoint -> Request -> Event

gateway:
Event -> Delivery -> Attempt -> Destination

test system:
Fixture / Scenario -> Event or Request -> Assertion

agent system:
Agent -> Callback/Endpoint -> Wait -> Event -> Verify -> Act
```

The market does not use these levels consistently, but the vocabulary changes noticeably as products move from development debugging to production reliability.

### Request != Event != Delivery != Attempt

This distinction survives the 102-product corpus and becomes stronger after adding gateways.

- **Request** is the observed protocol interaction.
- **Event** is the durable or semantic unit derived from an interaction.
- **Delivery** is an event being routed to a destination.
- **Attempt** is one execution of a delivery.

A request-bin can stop at Request. A delivery gateway cannot.

## Strongest co-occurrences

The most common concept pairs in the normalized corpus are:

| Pair | Products |
| --- | ---: |
| inspect + receive | 59 |
| inspect + replay | 45 |
| receive + replay | 42 |
| inspect + programmability | 41 |
| forward + inspect | 38 |
| programmability + receive | 36 |
| forward + receive | 35 |
| inspect + persist | 34 |
| inspect + provision | 33 |
| persist + receive | 28 |
| forward + replay | 28 |
| programmability + replay | 27 |
| persist + replay | 26 |
| inspect + verify | 25 |
| receive + tunnel | 24 |
| forward + persist | 23 |
| inspect + tunnel | 22 |
| replay + verify | 20 |
| forward + tunnel | 20 |
| inspect + retry | 19 |

This suggests that the canonical direct-product loop is no longer merely `capture -> inspect`. The center of gravity is closer to:

`provision -> receive -> inspect -> persist -> replay/forward`

## Six ontology layers

### 1. Boundary

Objects:
- Endpoint
- Callback

Actions:
- Provision
- Expire / claim / dispose
- Expose / tunnel

Question answered:
> Where can an external system reach me, and for how long?

### 2. Evidence

Objects:
- Request
- Response
- Payload
- History / recording

Actions:
- Receive / capture
- Inspect / search
- Persist
- Replay

Question answered:
> What actually crossed the boundary, and can I reproduce it?

### 3. Event and delivery

Objects:
- Event
- Route
- Destination
- Delivery
- Attempt
- Queue

Actions:
- Forward / route / fan-out
- Transform
- Retry / backoff / dead-letter
- Deduplicate

Question answered:
> How does captured external input become reliable application delivery?

### 4. Testing

Objects:
- Fixture
- Scenario

Actions:
- Simulate / mock
- Control response
- Replay
- Assert

Question answered:
> Can this integration behavior be reproduced and verified deterministically?

### 5. Agent

Objects:
- Agent
- Callback / inbox
- cursor/subscription state

Actions:
- Provision
- Wait / subscribe / resume
- Inspect
- Verify
- Act
- Cleanup

Question answered:
> Can autonomous software operate the integration-development loop without a human polling a dashboard?

### 6. Cross-cutting dimensions

- Programmability
- Durable point
- Actor model
- Trust boundary
- Reachability topology
- Collaboration
- Deployment model

These dimensions cut across product categories rather than belonging to one feature family.

## Durable point as the strongest architectural discriminator

The earlier local-vs-remote split is too weak.

A more useful classification is where the interaction becomes durable:

1. **No durable point** - direct tunnel to localhost.
2. **Capture durable point** - hosted request/inbox stores raw interaction.
3. **Event durable point** - request becomes a durable event.
4. **Delivery durable point** - event plus delivery/attempt state survives failures.
5. **Scenario durable point** - reusable fixture/scenario survives as test state.
6. **Agent inbox durable point** - event remains pending until an autonomous consumer advances a cursor/ack.

This axis explains why ngrok, Webhook.site, Hookdeck, PayloadGrid, Webhook Simulator and Agent Inbox can all overlap Hooktry while being ontologically different products.

## Agent synchronization is now a real primitive

`wait / subscribe / resume` appears in 14 products in the current summaries.

Examples include HookSense, webhook-capture-mcp, Agent Inbox and Kite.

The important distinction is:

```text
list_requests + polling
        !=
wait_for_request / wait_for_callback
```

The latter turns a webhook endpoint into a synchronization primitive for autonomous software.

That strengthens S001.

## Interaction as a Hooktry candidate abstraction

`Interaction` is deliberately included in the taxonomy with a market evidence count of zero.

This is not a claim that competitors use the term.

It is a candidate internal Hooktry abstraction:

```text
Interaction
  - Endpoint reference
  - Request
  - Response
  - timestamps/timing
  - raw evidence
  - derived Event
  - trust/verification evidence
```

Why keep it?

Because Hooktry wants to operate across external software boundaries, while market nouns fragment into Request, Event, Delivery and Attempt. An Interaction can preserve protocol truth without forcing the product to call every HTTP request an Event.

## Candidate Hooktry model after S003

```text
Workspace / Fixture
    |
Endpoint
    |
Interaction
    |- Request
    |- Response
    |- Evidence
    |- Verification
    |
    +-> Event
          |
          +-> Delivery
                |
                +-> Attempt
                |
                +-> Destination

Scenario
    |- Fixtures / recorded Interactions
    |- Response policy
    |- Assertions
    |- Failure simulation

Actor
    |- Human
    |- Script
    |- CI
    |- Agent
          |
          +-> provision / wait / inspect / verify / act / cleanup
```

This is a hypothesis, not yet a Hooktry implementation decision.

## What S003 changes

Before S003 the working category progression looked roughly like:

`request bin -> debugger -> relay -> gateway -> agentic fixture`

After normalizing the 102-product corpus, that linear progression is too simplistic.

A better model is multiple orthogonal layers:

```text
Boundary
Evidence
Delivery
Testing
Agent
+ cross-cutting trust/programming/topology
```

Products are compositions of these layers.

For example:

- ngrok: Boundary + Connectivity
- Webhook.site: Boundary + Evidence
- Splithook: Boundary + Evidence + Trust + Replay
- Hookdeck: Boundary + Evidence + Delivery
- PayloadGrid: Event + Delivery + Reliability
- Webhook Simulator: Testing + provider semantics
- HookSense: Boundary + Evidence + Agent synchronization
- Agent Inbox: Durable event/inbox + Agent synchronization

This compositional view should replace a single one-dimensional competitor ladder in future analysis.

## Cautions

- Counts are based on our current normalized summaries, not full product-by-product feature verification.
- `assert`, `dedupe`, `ordering`, and similar infrastructure semantics are likely undercounted.
- Market language over-represents UI-visible capabilities such as inspect/replay and under-represents implementation semantics.
- A low lexical count does not mean low strategic importance.
- S003 must stay frozen even if deeper research later changes counts or terminology.

## Next research questions

1. Which ontology rows are verified across the top 15-20 direct competitors rather than merely mentioned?
2. Does `Interaction` survive hands-on product comparison, or should Hooktry use `Event` or `Fixture` as the primary durable object?
3. Is `wait` important enough to become a first-class API primitive in Hooktry V1?
4. Does response control belong in the same object as Endpoint, Scenario, or Fixture?
5. Which concepts differentiate Hooktry versus becoming table-stakes?
6. Can the six-layer ontology cleanly classify all future discoveries without adding category-specific exceptions?
