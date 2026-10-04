# S006 - Category archaeology: from POST catcher to agent callback

**Snapshot ID:** S006  
**Captured:** 2026-10-04  
**Related discovery pass:** P19  
**Market Map after archaeology:** 206 products including Hooktry  
**Primary workbook output:** `18 Category Archaeology`  
**Status:** frozen historical synthesis - non-canonical

> S004 remains reserved for future hands-on Ontology Verification.

## Purpose

Reconstruct how webhook-related developer tooling evolved before the current market vocabulary existed.

The question is not only "which products existed?" but:

- which user problem appeared first;
- which primitive each generation introduced;
- which products died and why;
- which jobs were absorbed by larger platforms;
- which allegedly new 2026 ideas are actually old primitives under a new actor model.

## The core finding

The category did **not** evolve as one linear ladder.

It has at least two independent roots:

```text
ROOT A - Reachability
webhook receiver lives on localhost
    ->
public URL
    ->
reverse tunnel / relay
    ->
localhost

ROOT B - Evidence
there is no receiver yet, or I do not trust it
    ->
public bin / inbox
    ->
capture request
    ->
inspect retained evidence
```

Those roots later recombined with:

- programmable response behavior;
- request forwarding;
- realtime streams;
- API/mock tooling;
- reliable event delivery;
- standardized signing;
- autonomous actors.

## Timeline

### 2007 - webhook as a protocol idea

Jeff Lindsay coined/popularized the term "webhook" for user-defined HTTP callbacks.

The primitive is:

`event -> HTTP callback`

The original problem is avoiding polling, not debugging.

Source:
https://en.wikipedia.org/wiki/Webhook

### 2010 - localtunnel: the reachability root

Jeff Lindsay's localtunnel history preserves:

- prototype: early 2010;
- v1: mid-2010;
- v2: 2011-2013.

The problem was making a local web server reachable from the public internet with a friendly URL.

This creates the first major branch:

`public callback URL -> tunnel -> localhost`

There is no requirement for an independent durable event store.

Source:
https://github.com/progrium/localtunnel

### 2011 - PostBin: the evidence root

PostBin was already used for webhook testing in 2011.

Instead of reaching a real local application, the sender posts to a disposable public target and the developer inspects what arrived.

The product object becomes:

`Bin -> Requests[]`

Source:
https://stackoverflow.com/questions/7640814/is-is-possible-to-delete-a-postbin-org-bin

### 2011-2012 - PostBin becomes RequestBin

John Sheehan later described RequestBin as "the original POST catcher, formerly postbin.org."

The same Jeff Lindsay lineage therefore produced both foundational branches:

- localtunnel - reachability;
- PostBin/RequestBin - retained evidence.

Source:
https://www.john-sheehan.com/

### 2012 - early ecosystem, not one isolated project

By January 2012, RespondTo.it and RequestBin were both being used as dummy HTTP endpoints for inspecting requests.

This matters because the category already had multiple implementations before modern webhook-tool branding.

Source:
https://www.cantoni.org/2012/01/08/simple-webservice-echo-test/

### 2013 - ngrok normalizes localhost webhook development

A Twilio tutorial from October 2013 demonstrates ngrok specifically for testing webhook callbacks against local code.

This makes the "real provider -> local handler" loop mainstream:

`provider -> ngrok URL -> localhost`

Source:
https://www.twilio.com/en-us/blog/test-your-webhooks-locally-with-ngrok-html

### 2013 - WebhookInbox already has programmable wait semantics

WebhookInbox's API documentation contains a request example timestamped June 2013 and exposes a surprisingly modern model:

- create/destroy inbox;
- TTL;
- retrieve items;
- realtime stream;
- long-polling;
- programmatic API;
- custom HTTP responses;
- `response_mode=wait`.

In wait mode, WebhookInbox holds an inbound HTTP request until another API call supplies the response.

This changes an important S003 interpretation.

`wait` is **not an AI-era invention**.

What changes in 2026 is the actor:

```text
2013:
browser/script waits for callback state

2026:
autonomous agent provisions callback,
waits/resumes, verifies, acts, cleans up
```

Source:
https://webhookinbox.com/docs/api.html

### 2014 - webhook debugging is recognized as a distinct API-tool category

John Sheehan's 2014 API tooling survey has a specific "Webhook Debugging" section listing RequestBin, Webhook Inbox, RespondTo.it and others.

By this point, webhook debugging is a recognizable product job rather than an isolated utility.

Source:
https://www.john-sheehan.com/

### 2014 - realtime inspection becomes expected UX

Request Catcher uses a generated subdomain and pushes incoming requests to the open browser in realtime.

The UX moves from:

`send -> refresh -> inspect`

toward:

`send -> immediately appears`

Source:
https://requestcatcher.com/

### 2015 - PutsReq pushes beyond passive inspection

PutsReq combines:

- request recording;
- JavaScript-defined status/headers/body;
- request forwarding;
- localhost forwarding.

This is an early merger of:

`request bin + mock receiver + relay`

Source:
https://putsreq.com/

### 2015-2016 - Mockbin merges API mocking and bins

Mashape/Kong's Mockbin lets developers create custom endpoints, track HTTP traffic and define mock responses.

The market boundary between:

- request inspection;
- API mocking;
- webhook testing

is already porous.

Source:
https://github.com/Kong/insomnia-mockbin

### November 2016 - Webhook.site proves anonymous instant provisioning

Simon Fredsted describes posting Webhook.site to Hacker News in November 2016 and receiving roughly 4,000 unique users during the first traffic surge.

The winning UX is extremely small:

`visit -> get unique URL -> send -> inspect`

No project setup is required.

Source:
https://simonfredsted.com/1721

### 2017 - Beeceptor broadens the workbench

Beeceptor public beta in October 2017 combines:

- API mocking;
- proxying;
- recording;
- custom responses;
- failure/latency simulation;
- webhook endpoints.

This reinforces the convergence of webhook testing with general integration-development tooling.

Source:
https://devpost.com/software/beeceptor

### 2017 - Webhook Relay formalizes private-network delivery

Webhook Relay describes a different problem: public providers must notify CI/internal systems that cannot accept public inbound traffic.

The flow becomes:

`provider -> relay -> private/internal destination`

with filters, transforms and later multiple destination types.

Source:
https://webhookrelay.com/blog/introduction/

### early 2018 - smee.io specializes the development relay

A February 2018 Probot issue already references smee.io as the webhook proxy for local GitHub App development.

Smee's architecture deliberately avoids a server-side durable payload store; it forwards to connected clients and uses browser localStorage for visible history.

This is another strong durable-point distinction.

Source:
https://github.com/probot/probot/issues/424  
https://github.com/probot/smee.io

### March 21, 2018 - original hosted RequestBin dies from abuse

Runscope discontinued the public RequestBin instance because ongoing abuse made it difficult to keep the service reliable.

The source remained self-hostable.

This is one of the most important product-history signals in the category:

**anonymous arbitrary public HTTP ingestion creates a structural abuse and cost problem.**

Source:
https://stackoverflow.com/questions/5725430/http-test-server-accepting-get-post-requests/9770981

### 2018 - Hookbin repeats the same operational failure mode

Hookbin also went offline because of extreme abuse and was later rewritten/relaunched.

This shows RequestBin's failure was not an isolated company mistake.

Source:
https://css-tricks.com/hookbin-capture-inspect-http-requests/

### August 2019 - RequestBin.com revives the lightweight job

A new RequestBin.com was launched as a modern take on the old RequestBin.

Its founders emphasized:

- persistent HTTPS endpoints;
- realtime stream;
- authentication/privacy;
- filtering;
- event deletion;
- reliable hosting.

The underlying job remained valuable after the original service disappeared.

Source:
https://news.ycombinator.com/item?id=20758684

### 2020-2021 - request bin becomes programmable event source

Pipedream wrote in January 2020 that more than 100,000 developers had used RequestBin.com and showed how an HTTP trigger plus code/state could provide an Events API.

By 2021 Pipedream was directing new RequestBin users toward Pipedream HTTP sources.

The transition is:

`capture -> inspect`

to

`capture -> code -> transform -> state -> deliver`

Source:
https://pipedream.com/blog/requestbin-events-api/  
https://pipedream.com/community/t/requestbin-data/454

### 2021 - reliability infrastructure becomes a product category

Hookdeck, Svix and Convoy represent a new wave.

The problem is no longer only developer visibility. It is production correctness:

- ingest reliably;
- persist;
- queue;
- verify;
- retry;
- monitor;
- replay;
- fan out;
- track delivery attempts.

Hookdeck's 2021 launch explicitly frames the choice as either building ingestion/queueing/processing/monitoring/alerting yourself or using dedicated webhook infrastructure.

Svix focuses on sender-side webhooks-as-a-service.

Convoy focuses on reliable inbound/outbound webhook proxy/gateway infrastructure.

Sources:
https://news.ycombinator.com/item?id=28063597  
https://www.ycombinator.com/companies/svix  
https://www.ycombinator.com/companies/convoy-2

This is where the object hierarchy becomes unavoidable:

`Request != Event != Delivery != Attempt`

### December 2023 - webhook standardization

Standard Webhooks is announced to reduce provider-specific fragmentation in signing, security and interoperability.

The category now has enough maturity that common protocol conventions become a separate layer.

Source:
https://www.svix.com/blog/standard-webhooks/

### December 2024 - standalone RequestBin is absorbed

Pipedream confirms that legacy RequestBin was shut because maintaining two products in parallel was difficult and its functionality had been consolidated into Pipedream.

This is an important caution:

a broader platform can contain more capability while still making the original lightweight job worse for users who wanted only:

`one click -> URL -> payload`.

Source:
https://pipedream.com/community/t/where-can-i-find-the-legacy-version-of-http-requestbin-com-requestbin-com/12091

### 2026 - surviving bins expand into automation

Webhook.site now describes itself as no longer only a webhook inspector. It has added workflow automation, transformations, schedules and databases while preserving the instant unique-URL experience.

Source:
https://webhook.site/blog/2026-07-15-what-is-webhooksite

### 2026 - autonomous actors become first-class

The 200-product census found products and adjacent workflow systems where callback/wait is explicitly agent-addressable.

The new loop is:

```text
agent provisions callback
    ->
external async work
    ->
event arrives
    ->
agent wait resumes
    ->
inspect / verify
    ->
act
    ->
cleanup or retain evidence
```

The primitive is old.

The **actor model, lifecycle ownership, evidence requirements and autonomous orchestration are new**.

## The revised evolutionary model

The old linear model:

`request bin -> debugger -> relay -> gateway -> agentic fixture`

is historically wrong.

A better model is a branching graph:

```text
                         +------------------+
                         | Webhook protocol |
                         |      2007        |
                         +---------+--------+
                                   |
                    +--------------+--------------+
                    |                             |
                    v                             v
             REACHABILITY                    EVIDENCE
             localtunnel                     PostBin
                2010                           2011
                    |                             |
           ngrok / UltraHook                 RequestBin
                    |                             |
                 smee.io              WebhookInbox / Catcher
                    |                             |
                    +-------------+---------------+
                                  |
                                  v
                         DEBUGGING WORKBENCH
                   response / mock / forward
                      PutsReq / Mockbin
                                  |
                +-----------------+-----------------+
                |                                   |
                v                                   v
         AUTOMATION / WORKFLOW              RELIABILITY / GATEWAY
           Pipedream / site                 Hookdeck / Convoy
                                                    |
                                           sender infrastructure
                                                  Svix
                                                    |
                                           protocol standards
                                       Standard Webhooks 2023
                +                                   +
                +-----------------+-----------------+
                                  |
                                  v
                           AGENTIC ACTOR
                     provision / wait / verify
                        act / cleanup / evidence
```

## Product-history lessons for Hooktry

### 1. Anonymous-first is powerful and dangerous

The instant anonymous URL repeatedly wins on time-to-value.

It also repeatedly creates abuse, retention, cost and accidental-production-data problems.

Hooktry's anonymous-first model therefore needs explicit:

- TTL;
- quotas;
- capability separation;
- rate/abuse controls;
- safe retention defaults;
- clear production warnings;
- claim/ownership transition.

These are not polish.

They are historically proven survival requirements.

### 2. Keep the lightweight job even if the platform grows

RequestBin -> Pipedream shows that feature consolidation can lose the emotional value of the original one-click workflow.

Webhook.site shows the opposite strategy: retain instant capture, then add automation around it.

Hooktry should prefer:

`simple core -> optional deeper control`

over forcing every user into the larger control plane.

### 3. Durability is the best architecture axis

History repeatedly splits tools by where state survives:

- tunnel: local process/session;
- Smee: connected client/browser;
- bin: hosted request store;
- workflow: event/state;
- gateway: event + delivery + attempt;
- fixture/scenario: reproducible test state;
- agent inbox: resumable actor state.

This remains stronger than the local-vs-remote classification.

### 4. Response control is old and important

WebhookInbox and PutsReq show that controlling the receiver response was an early need.

It should not be treated as an exotic modern feature.

For Hooktry it naturally belongs near Fixture/Scenario semantics.

### 5. Agentic differentiation must be more than MCP

Because API, stream, long-poll and wait primitives existed long before AI, an MCP wrapper alone is not a new category.

A genuinely agent-native receiver should improve the full loop:

- provision safely;
- return typed capabilities;
- wait without wasteful polling;
- correlate the right interaction;
- expose trustworthy raw evidence;
- verify signatures;
- control/replay responses;
- assert outcomes;
- clean up resources;
- preserve proof when needed.

## Corpus changes made during archaeology

P19 added six historically important products/lineages to Market Map:

1. original PostBin / RequestBin lineage;
2. WebhookInbox;
3. RespondTo.it;
4. PutsReq;
5. Mockbin;
6. UltraHook.

The Market Map therefore moves from 200 to **206** rows.

These additions were not made to increase the census number. Each was added because it materially explains a primitive or lineage that the 200-product present-day census obscured.

## Next implication

S004 hands-on Ontology Verification should use both:

- S003 ontology;
- S006 historical corrections.

In particular, S004 should test whether modern products implement old primitives in materially better ways, rather than treating every current feature name as novel.
