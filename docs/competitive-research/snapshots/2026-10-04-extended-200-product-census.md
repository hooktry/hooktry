# S005 - Extended 200-product census and saturation probe

**Snapshot ID:** S005  
**Captured:** 2026-10-04  
**Source corpus:** Market Map with 200 unique product URLs including Hooktry  
**Related discovery passes:** P13-P18, building on P00-P12  
**Status:** frozen discovery snapshot - non-canonical

> S004 is intentionally reserved for the future hands-on Ontology Verification research slice defined in `docs/rfc/competitive-ontology-verification.md`.

## Purpose

Test whether the 102-product census in S002 was already close to market saturation or whether poor-SEO, GitHub-only, self-hosted, agent-adjacent and provider-native searches would continue to produce meaningful candidates.

The goal was not to manufacture a round number. The goal was to observe the transition from new market signal to repetitive search noise.

## Result

The Market Map expanded from **102 to 200 products including Hooktry**.

All 200 rows have unique Product URLs.

P13-P18 added **98 unique candidates**:

- P13 deep direct / poor-SEO inspector tail: 34
- P14 local reachability / tunnel deep tail: 18
- P15 agent callback / durable wait deep tail: 10
- P16 mock / simulation / debugging ecosystem: 12
- P17 gateway / relay / OSS infrastructure deep tail: 19
- P18 provider-native second wave: 5

The detailed exact-query provenance, duplicates, exclusions, saturation signal and noise pattern are recorded in Google Sheet tab `17 Discovery Passes`.

## Main conclusion

**102 products was not saturated.**

A deeper search still surfaced multiple products with genuinely new or strategically useful ideas rather than only clones.

Examples:

- **HookCapsule** - capture a real webhook failure, redact before persistence, preserve an ordered multi-event capsule, replay it, and export a Vitest regression test.
- **OpenWebhook / HookTray / ReHook** - different local-first privacy models where the durable evidence can live in browser IndexedDB or local SQLite rather than a hosted payload database.
- **WebhookX / HookTrace** - explicit webhook control-plane object models spanning receive, store, queue, deliver, retry and replay.
- **Exende Callback API** - callback endpoint as an API primitive with wait/read/delete semantics for scripts and agents.
- **Upstash Workflow / Orkes / agentic orchestrators** - wait-for-webhook as durable workflow suspension rather than request-list polling.
- **Mercur / Burrow** - tunnel products moving toward history, replay and MCP rather than pure reachability.

These finds materially strengthen the S003 ontology instead of merely increasing competitor count.

## Search-surface finding

The discovery surface matters as much as the query vocabulary.

The 102-product census was mostly web-search driven.

P13-P18 deliberately added:

1. normal web search with long-tail query variants
2. direct GitHub repository search
3. GitHub result pages beyond the first page
4. package registries
5. IDE/extension marketplaces
6. first-party provider documentation
7. workflow/agent products that do not primarily call themselves webhook tools

Direct GitHub repository search was especially useful for products with almost no SEO.

## Saturation by family

### Direct webhook inspector / debugger

**State: approaching saturation, but not exhausted.**

P13 still found 34 unique rows, including several differentiated products.

However GitHub page 2 for generic terms such as `webhook inspector` and `webhook debugger` changes character sharply:

- many repositories have nearly identical names
- many are tutorial/challenge/student implementations
- product positioning is absent
- differentiation collapses to framework choice
- many have no hosted surface, release story or novel workflow

This is the clearest point where raw result count stops being equivalent to useful market signal.

Strong low-SEO exceptions still exist, so future discovery should use evidence filters rather than simply stop at a page number.

### Local tunnel / reachability

**State: moderately saturated.**

There are many generic ngrok alternatives.

Pure reachability now adds little ontology value.

A tunnel becomes strategically interesting for Hooktry when it adds one or more of:

- durable request history
- replay
- webhook signature semantics
- stable endpoint identity
- capture-first behavior while local machine is offline
- MCP/agent access

### Agent callback / durable wait

**State: not saturated semantically, but low-volume.**

The result set is smaller than generic webhook tooling, but the signal is unusually high.

Search terms such as:

- callback endpoint
- wait for webhook
- wait for event
- resume workflow
- no polling
- webhook to agent

surface products outside the traditional webhook category.

This continues to support `wait_for_request / wait_for_callback` as a genuine synchronization primitive.

### Mock / simulation / service virtualization

**State: category is huge, ontology coverage is becoming saturated.**

There are many mock-server products, but most repeat the same primitives.

The strategically relevant additions are:

- contract-driven callback/webhook mocks
- provider-shaped signed simulation
- programmable response behavior
- request verification/assertions

Counting every generic API mock would inflate the census without improving Hooktry understanding.

### Gateway / relay / reliability OSS

**State: mixed.**

The first page of repository search still exposes architecture-rich projects such as WebhookX and HookTrace.

Deeper generic relay searches quickly degrade into:

- GitHub-to-X bridges
- Discord/Slack relays
- Alertmanager receivers
- one-provider adapters
- short scripts

Those are legitimate software but usually not useful competitors for Hooktry's product/job boundary.

### Provider-native tooling

**State: strategically saturated.**

After Stripe, GitHub, Shopify, Square, Paddle, Twilio, PayPal, GitLab, Supabase and similar examples, adding another provider usually repeats one of:

- trigger a test event
- simulate a provider-shaped event
- inspect recent deliveries
- redeliver an event
- forward events to localhost

This population is effectively unbounded because every webhook provider can build one.

Future provider-native additions should be included only when they introduce a new primitive or unusually strong UX pattern.

## Noise onset

The strongest observed noise patterns were:

### Generic repository clones

Searches such as `webhook inspector` and `webhook debugger` produce dozens of minimally differentiated repositories on later GitHub pages.

These should not automatically enter Market Map.

### Lexical collisions

`request bin webhook` begins returning Binance/FrostyBot repositories because of the substring `bin`.

This is useful negative evidence about query quality.

### Narrow relays

`webhook relay` produces many useful pieces of software that only translate one provider to one destination.

These are implementation references, not necessarily market competitors.

### Templates and tutorials

A code template or tutorial can demonstrate an architecture without constituting a product.

Keep such items in exclusion notes unless they provide a distinct ontology primitive.

## New inclusion rule after S005

For future breadth discovery, a new candidate should normally satisfy at least one of:

1. distinct product or maintained open-source tool with a coherent user job
2. new ontology primitive or unusual composition of known primitives
3. meaningful agent/CI/programmatic surface
4. meaningful trust, durability, replay, scenario or response-control semantics
5. evidence of product commitment: current docs, releases, pricing, API, CLI, MCP, package or deployment story
6. historically important product needed to explain category evolution

Do not add a repository only because its name matches the query.

## Saturation heuristic

The census is now sufficiently broad for deeper research.

This does **not** mean that every relevant product has been found.

It means the marginal search result increasingly falls into one of three classes:

```text
duplicate pattern
narrow implementation
low-differentiation clone
```

while genuinely new ontology patterns appear much less frequently.

A future breadth pass is justified when it changes the search surface or ontology, for example:

- non-English ecosystems
- new agent/event terminology
- new protocol family
- newly launched products
- historical/abandoned category archaeology

It is not justified merely to push the count from 200 to 300.

## Relationship to S003

S003 remains frozen as the ontology extracted from the 102-product corpus.

Do not silently recompute its counts against 200.

S005 provides a larger selection universe and saturation evidence.

The future S004 Ontology Verification should:

- use the S003 ontology as the hypothesis under test
- select strategic hands-on subjects from the larger S005 200-product corpus
- verify behavior rather than marketing vocabulary

## High-priority candidates added after S003

Candidates that appear especially useful for future hands-on selection include:

- HookCapsule
- OpenWebhook
- HookTray
- ReHook
- HookPilot
- Webcatch
- Burrow
- Mercur
- Exende Callback API
- Requestify
- WebhookX
- HookTrace
- Hooksbase
- Chirp Hookbox

This is not yet the S004 shortlist. It is only a high-signal candidate pool.

## Bottom line

The market is larger and more heterogeneous than the initial top-search results suggested.

At 200 products, the research is finally showing a meaningful saturation curve:

```text
obvious leaders
    ->
long-tail SaaS / focused utilities
    ->
poor-SEO indie + OSS products
    ->
adjacent agent/testing/reliability systems
    ->
generic clones / narrow bridges / provider repetition
```

The useful discovery frontier now depends more on **new vocabulary and new search surfaces** than on going one more page deeper with the same generic query.
