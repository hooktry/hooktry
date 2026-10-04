# S002 - Discovery passes P04-P12

**Snapshot ID:** S002  
**Captured:** 2026-10-04  
**Parent hypothesis:** S001 - agent-addressable public event receiver  
**Market Map size:** 102 products including Hooktry  
**Status:** frozen research snapshot - non-canonical

## Purpose

Preserve not only the products found, but the search language that exposed them.

This snapshot starts the strict provenance phase of Hooktry competitive research. Every discovery pass records its query family, exact search terms, new products, duplicates, false positives, and exclusions. Each Market Map row discovered in these passes carries a Discovery Pass, Discovery Query, Discovery Family, Maturity Class, and Maturity Signal.

P00-P03 were performed before strict query provenance was introduced. They are retro-tagged honestly as `not captured (pre-provenance)`; their exact historic queries are not reconstructed.

## Reproducibility note

Search engines may merge results from several queries in one research batch. Therefore:

- the pass stores every exact query issued;
- the product row stores the best matching query that led to classification;
- we do not claim a search engine exposed a product from one exact query when the response was merged;
- false positives are preserved because they measure how search language leaks into adjacent categories.

## Discovery passes

| Pass | Query family | Exact queries | New products |
| --- | --- | --- | --- |
| P04 | Agent-native / AI-adjacent | `"MCP request bin"`; `"MCP webhook" "wait for" request`; `"AI agent" "request bin"`; `"AI agent" webhook inbox`; `"agent-native" webhook`; `"agentic" webhook testing`; `"webhook to agent" developer platform`; `"agent inbox" webhook API developer tool` | Agent Inbox; Kite; TestSprite; Activepieces Webhook MCP |
| P05 | Catch / inspect / replay / inbox | `"webhook inbox" replay localhost`; `"webhook catcher" localhost replay`; `"webhook logger" inspect replay`; `"requestbin alternative" webhook`; `"temporary webhook endpoint" inspect replay`; `"webhook debugger" replay localhost`; `"webhook monitor" replay developer` | Webhook Trap; PeekAt; HookNexus; RelayStack; RelayFox; CatchKit HookCatch; Hookdump; Kordu Webhook Tester; DevTools Webhook Inbox; Stockyard Corral |
| P06 | Local tunnel / reachability | `"webhook tunnel" localhost testing`; `local webhook testing tunnel`; `webhook proxy localhost` | rustunnel; Taupi; SteadIP |
| P07 | Gateway / reliability / delivery | `"webhook gateway" developer platform retries replay`; `"webhook delivery" platform retries signatures replay`; `"webhook infrastructure" developer platform send receive`; `"webhook delivery platform" open source`; `"webhook observability" platform` | webhook-gateway; Sparrow; Hooktrack; PayloadGrid; hookdash; thealirazadev/hook-relay; Kook; whook; Webhook Watchtower |
| P08 | Provider-native substitutes | `Stripe webhook local testing`; `site:docs.github.com webhook testing/redelivery`; `site:shopify.dev webhook trigger`; `Square webhook subscription test`; `Paddle webhook simulator` | Stripe CLI; GitHub webhook testing/redelivery; Shopify CLI webhook trigger; Square test webhook subscription; Paddle Webhook Simulator |
| P09 | Agent callback / MCP-native | `"webhook inbox" MCP developer`; `"webhook capture" MCP server`; `"webhook tester" "MCP"`; `"callback endpoint" "MCP" agent`; `"webhook callback" "wait_for_callback"` | HookSense; webhook-capture-mcp; FlurryPORT |
| P10 | Sandbox / playground / request inbox | `"webhook sandbox" developer tool`; `"webhook playground" testing tool`; `"callback catcher" webhook developer tool`; `"HTTP request catcher" webhook inspector`; `"request inbox" HTTP developer`; `"webhook sandbox" replay localhost` | Zunoy Webhook Sandbox; MockFlow Request Catcher; Outworx Webhooks Playground; Convoy Playground; HooksEasy; Request Inbox |
| P11 | Reliability layer / durable ingest | `"webhook reliability gateway" developer`; `"webhook reliability" gateway retries replay`; `"durable webhook" gateway developer`; `"webhook reliability" platform ingest persist replay`; `"inbound webhook gateway" persist retry replay` | GetHook; Hooklayer; event-bridge |
| P12 | Self-hosted inbox implementations | `"webhook inbox" API replay`; `"HTTP request inbox" webhook`; `self-hosted webhook inbox capture HMAC replay` | hookbox |

P04-P12 added **44 products** to the prior 58-product map, bringing the corpus to **102 products including Hooktry**.

## Preserved false positives and exclusions

These are evidence about the search boundary, not wasted searches.

- `ai-hook` - agent security hook/rule dispatcher; not an event receiver.
- Agent-Native framework results - broad agent architecture, not a callback/webhook product.
- Email-only agent inbox products - adjacent naming, wrong event boundary for this pass.
- Generic reliability articles and comparison posts - useful vocabulary, but no product.
- Xano `webhook-inbox` template - useful reference implementation, excluded from product count because it is a template rather than a standalone product.
- Generic API senders with no public receiving endpoint - excluded from catcher/sandbox passes.

## Territory exposed by search language

### 1. Request-bin language

`catcher`, `inbox`, `inspector`, `debugger`, and `request bin` expose the classic development workflow:

`receive -> inspect -> replay/forward`

This remains the densest direct-competitor cluster.

### 2. Tunnel language

`tunnel`, `proxy`, and `localhost` expose a different job:

`make local code reachable`

These products can substitute for Hooktry during development even when they do not persist an event independently of the developer machine.

### 3. Reliability language

`reliability layer`, `durable ingest`, `gateway`, `persist`, `retry`, and `dead-letter` expose the production-side evolution:

`verify -> persist before ACK -> queue -> deliver -> retry -> dead-letter -> replay -> observe`

The durable object is increasingly an event plus delivery attempts, not merely an HTTP request.

### 4. Agent language

`callback`, `wait`, `agent inbox`, `MCP`, and `wake-up` expose a category that is not naturally found by searching for "webhook tester".

The emerging loop is:

`agent provisions callback -> external async work runs -> event is durably captured -> agent waits/resumes -> verifies -> acts -> disposes or retains evidence`

This is stronger evidence for the S001 hypothesis than a simple MCP wrapper around a conventional dashboard.

### 5. Sandbox / playground language

`sandbox`, `playground`, and `inbox` expose products that frame webhook traffic as shared test state rather than networking infrastructure. Team visibility, retained evidence, provider simulation, signing, and replay appear frequently here.

### 6. Provider-native tooling

Stripe, GitHub, Shopify, Square, and Paddle provide their own testing, replay, trigger, or simulation surfaces. These are substitutes rather than standalone webhook-tool companies.

This means a generic webhook tool competes not only with other webhook products, but with the provider's own development loop.

## Maturity taxonomy used in this snapshot

Maturity is deliberately separate from feature coverage.

- **Thin utility** - a narrow usable tool with little evidence of a larger product commitment.
- **OSS prototype** - runnable and relevant, but limited evidence of durable product investment or adoption.
- **Active OSS project** - maintained implementation with coherent architecture/docs.
- **Active indie** - current product with a real user surface, but limited evidence of organizational scale.
- **Committed new player** - new/early product with meaningful investment signals such as pricing, docs, API/CLI/MCP, changelog, or coherent product architecture.
- **Established / active** - mature broader product or company with an active webhook surface.
- **Established provider-native** - first-party tooling from the webhook provider itself.
- **Legacy / status unknown** - relevant surface exists, but current development/commercial status needs verification.

These are research classifications, not judgments about product quality.

## Working observations

1. The market cannot be mapped with the noun `webhook` alone. `callback`, `inbox`, `sandbox`, `playground`, `event delivery`, and `reliability layer` expose different nearby populations.
2. `wait_for_callback` / `wait_for_request` is becoming a recognizable agent synchronization primitive, not merely a convenience API.
3. A useful axis is **where the durable point lives**: nowhere/tunnel, hosted capture, event store, delivery queue, test scenario, or agent inbox.
4. Provider-native testing tools remove part of the value proposition of generic webhook products for single-provider workflows.
5. The long tail is not composed only of student projects. It contains thin utilities, current indie SaaS, active OSS gateways, provider-native tooling, and explicitly agent-native new entrants.
6. Product identity must be URL-first. Repeated names such as HookRelay/hook-relay refer to multiple unrelated products and implementations.

## Questions for the next snapshot

- Does agent-native callback handling converge on webhook tooling, durable inboxes, or general event/messaging infrastructure?
- Is `wait` a primitive that should be part of Hooktry's core model rather than only MCP ergonomics?
- Should Hooktry's durable object be Endpoint, Interaction, Event, Fixture, Session, or Inbox?
- Which direct competitors have enough evidence of traction to deserve deep hands-on benchmarking?
- Which provider-native workflows set the UX baseline users will expect Hooktry to beat?
