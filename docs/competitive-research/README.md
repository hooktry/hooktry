# Hooktry competitive research

This directory defines the durable methodology for comparing Hooktry with direct competitors, adjacent tools, and substitute workflows.

## Working spreadsheet

Google Sheet: https://docs.google.com/spreadsheets/d/1tsKDL6RlVcpbEhes1muwzY3iEK_Kl8yQtnmBI8QhLGQ/edit

The workbook is the operational research surface. This document is the canonical description of how the research is structured and how evidence becomes product decisions.

## Research model

The research has three layers:

1. **Market facts** - what products exist, what jobs they serve, how they are delivered, what capabilities and limits they document.
2. **Interpretation** - overlap, gaps, workflow friction, cohort fit, positioning, and opportunities.
3. **Hooktry decisions** - explicit product choices with rationale and supporting evidence.

Do not collapse these layers into one feature-comparison table.

## Source hierarchy

Use sources in this order:

1. First-party product surface - landing pages, pricing, docs, API references, changelogs, security pages, integrations, and auth/signup flows.
2. Hands-on product testing - create endpoints, send requests, inspect payloads, test replay/forwarding/mock responses, and measure time-to-value.
3. Public technical footprint - GitHub repositories, SDKs, CLI, Docker/self-hosted distributions, packages, extensions, MCP/plugins, and integrations.
4. Historical evidence - changelogs, release notes, repository history, and archived pricing/features when history matters.
5. External sources - Reddit, Hacker News, Product Hunt, G2/Capterra, YouTube, and third-party comparisons. Use mainly for pain-point discovery and competitor discovery, not as authoritative feature/pricing evidence.
6. Hooktry itself - evaluate Hooktry through the same taxonomy and workflow benchmarks as every competitor.

## Research pipeline

`discover -> classify -> inspect first-party sources -> hands-on test -> capture evidence -> normalize features/jobs/workflows -> compare with Hooktry -> derive opportunities -> record decisions`

Research should be evidence-first. A non-obvious feature, limit, price, or product claim should eventually point to an evidence record.

## Research snapshots

Dated files under `snapshots/` preserve hypotheses, discovery states, and category definitions exactly as they were understood at a point in time. Snapshots are intentionally not canonical truth: later research may refine or contradict them. Keep the earlier snapshot unchanged so competing research threads can be compared before conclusions are consolidated.

Snapshots have stable `Sxxx` IDs and are indexed in `snapshots/README.md`. Discovery passes have stable `Pxx` IDs. The workbook's `17 Discovery Passes` tab correlates exact queries and results to snapshots, while each Market Map row records the pass/query/family that discovered it. Historical passes that predate query logging are explicitly marked `not captured (pre-provenance)` rather than reconstructed.


## Workbook structure

| Tab | Purpose |
| --- | --- |
| 00 Market Map | Classify products and determine who belongs in the market |
| 01 Competitor Matrix | Compact side-by-side summary |
| 02 Feature Taxonomy | S003 market ontology: canonical objects, actions, dimensions, lexical evidence counts, and interpretations |
| 03 Feature Coverage | Competitor x feature evidence |
| 04 Jobs & Use Cases | Why users open these products |
| 05 Workflow Benchmarks | End-to-end task friction and time-to-value |
| 06 Pricing | Plans, limits, free tiers, and pricing mechanics |
| 07 UX & Product | Signup, onboarding, inspection, history, sharing, and related UX |
| 08 Developer Experience | APIs, CLI, SDKs, docs, integrations, MCP/AI |
| 09 Architecture & Limits | Retention, payload/rate limits, protocols, and security characteristics |
| 10 Positioning & Distribution | ICP, messaging, SEO, GitHub, integrations, community, and acquisition surfaces |
| 11 Evidence | Source-of-truth claims and URLs |
| 12 Hooktry Gap Map | Parity gaps, intentional omissions, differentiators, commodities, and unknowns |
| 13 Opportunities | Product opportunities derived from evidence |
| 14 Decision Ledger | Hooktry decisions with rationale and evidence |
| 15 Change Log | Material competitor changes over time |
| 16 Research Queue | Open questions and next research actions |
| 17 Discovery Passes | Reproducible search passes: exact queries, additions, duplicates, exclusions, false positives, counts, snapshot correlation, and saturation/noise signals |
| 18 Category Archaeology | Historical milestones: category branches, primitives introduced, failure pressures, successor patterns, and Hooktry implications |

## Source-of-truth bias

Start from these tabs before filling derived comparison tabs:

- **00 Market Map**
- **02 Feature Taxonomy**
- **11 Evidence**
- **16 Research Queue**

The matrix, coverage, gap map, opportunities, and decisions should be populated from normalized research rather than manually painted conclusions.

## 00 Market Map

The Market Map separates **company** from **product** and answers a different question from the feature matrix: what kind of competitor is this, and why does it matter?

Current schema:

`Company | Product | Product URL | Parent Company | Status | Category | Directness | Primary Job | Primary Cohort | Secondary Cohort | Delivery Model | Product Motion | Hooktry Overlap | Distinctive Angle | Source URL | Source Type | Observed At | Confidence | Needs Recheck | Research Status | Notes | Discovery Pass | Discovery Query | Discovery Family | Maturity Class | Maturity Signal`

### Directness

- **Baseline** - Hooktry itself.
- **Direct** - solves substantially the same primary job for the same or very similar cohort.
- **Adjacent** - overlaps meaningful capabilities or workflows but has a broader/different primary job.
- **Substitute** - users can solve the same immediate problem with a materially different product category or workflow.

Do not treat every product in the map as an equivalent competitor.

### Primary cohort

Hooktry's current primary cohort is product development, testing, and pilot work. Secondary vectors can be explored separately rather than distorting the primary comparison.

## Evidence model

Canonical evidence fields:

`Evidence ID | Competitor | Claim | Source Type | Source URL | Observed At | Confidence | Needs Recheck | Notes`

Important distinctions:

- **Observed At** means when we checked the claim.
- **Last Material Change** means when the competitor materially changed.
- These are different signals.
- Confidence is about evidence quality, not how strongly we feel about a conclusion.
- Ambiguous current status should be marked for recheck instead of guessed.

## Controlled vocabularies

Recommended enums:

- Status: `Active | Early Development | Maintenance | Deprecated | Unknown`
- Directness: `Baseline | Direct | Adjacent | Substitute`
- Support Level: `Yes | Partial | No | Unknown | Deprecated`
- Confidence: `High | Medium | Low`
- Needs Recheck: `Yes | No`
- Research Status: `Baseline | Seeded | Needs status verification | Needs deeper research | Verified`
- Gap Type: `Parity gap | Intentional omission | Differentiator | Commodity | Unknown`

## Workflow benchmark

A canonical first workflow to benchmark across direct competitors:

`landing page -> webhook URL -> send request -> inspect request -> replay/share`

Feature parity alone is not enough. Hooktry may win or lose on the complete workflow even when the same boxes are checked.

## Market discovery status

The Market Map is intentionally broader than a shortlist of category leaders. It includes direct competitors, adjacent webhook infrastructure, substitute workflows, small focused request-bin products, and self-hosted/open-source alternatives.

As of 2026-10-04 the map contains **206 products including Hooktry**. S005 remains the frozen 200-product saturation census; P19/S006 deliberately added six historical lineages because they explain category primitives that the present-day census obscured.

The initial leader-focused pass covered products such as Webhook.site, Hookdeck, Beeceptor, Webhook Relay, Svix Play, Pipedream/RequestBin, ngrok, smee.io, Postman, Insomnia, Requestly, and Mockoon.

A second discovery pass expanded the long tail with products including:

- HookRelay
- webhooks.sh
- webhooks.cc
- HookCap
- Hooklistener
- Hook0 Play
- Webhooker
- Bluejay Relay
- UseWebhook
- WebhookScout
- req.is
- RequestBin.net
- HookBin
- Request Inspector
- Webhook.cool
- Request Catcher
- PostBin
- WebhookBin.net
- API Alerts Hooks
- tarampampam/WebHook Tester
- Convoy
- Hook0 Webhook Service

This second pass matters because several of the closest strategic overlaps are not the largest brands. Agent-first/API-first products such as HookRelay, webhooks.cc, Hooklistener, and WebhookScout should receive deeper research alongside the established webhook tools.

A third breadth-first discovery pass on 2026-10-04 added 23 more products, including Hook Relay (`hookrelay.dev`), HookRelay (`hookrelay.co`), Splithook, WebhookWhisper, WebhookSpy, Requex.me, WebhookCatch, Slashbin, Conduit, RelayBird, Axel, PomeloHook, Webhook Toolkit, webhook.co, Chis, HookRay (`hookray.com`), Hookray (`hookray.dev`), hookee, Webhooks.io, ViewHook, Testhooks, Webhook Simulator, and Hooksterr. This pass intentionally includes current indie/open-source utilities, production relay/gateway products, and early agent-native entrants rather than filtering only for established companies.

Strict query provenance starts at P04. P04-P12 added another **44 products** through deliberately different query languages: agent-native callbacks/MCP, catcher-inspector-replay workflows, localhost tunnels, reliability gateways, provider-native substitutes, sandboxes/playgrounds/inboxes, and self-hosted inbox implementations. The resulting 102-product census is frozen as snapshot **S002** in `snapshots/2026-10-04-discovery-passes-p04-p12.md`.

The passes also preserve exclusions and false positives. For example, `ai-hook` was found through AI-hook language but classified out because it is an agent security hook dispatcher rather than a public event receiver. This negative evidence is kept so future passes can compare query precision instead of silently discarding misses.

P13-P18 expanded the corpus from 102 to 200 unique Product URLs using deeper web search, direct GitHub repository search, package/extension surfaces, agent callback language, mock/simulation ecosystems, gateway/relay OSS, and a second provider-native pass. Snapshot **S005** freezes this census and the observed transition from useful long-tail signal to clone/provider noise.

P19 adds category archaeology rather than another generic breadth pass. It adds the original PostBin/RequestBin lineage, WebhookInbox, RespondTo.it, PutsReq, Mockbin, and UltraHook, and materializes 24 historical milestones in `18 Category Archaeology`. Snapshot **S006** preserves the resulting historical synthesis.

Pipedream / RequestBin remains intentionally marked for status verification. HookRelay, UseWebhook, Request Catcher, and API Alerts Hooks also have explicit follow-up questions in the Research Queue rather than guessed conclusions.

## Research guardrails

- Prefer first-party evidence for pricing, limits, current capability, and availability.
- Use third-party sources to discover pain points, alternatives, and market language.
- Do not infer missing capabilities from silence in marketing pages.
- Do not convert unknowns into "No".
- Separate a company's portfolio from the specific product being compared.
- Keep raw observations separate from Hooktry product decisions.
- Record deliberate non-features as decisions, not as accidental gaps.
- Revisit high-impact claims when their evidence becomes stale.

## Current status

As of 2026-10-04:

- `00 Market Map` contains 206 products including Hooktry after six archaeology-only lineage additions; S005 preserves the exact 200-product saturation census.
- Market Map rows include discovery provenance and provisional maturity signals.
- `17 Discovery Passes` records P00-P19; strict exact-query provenance begins at P04, P13-P18 add explicit saturation/noise fields, and P19 is the historical lineage pass.
- Snapshot S001 preserves the agent-addressable receiver hypothesis; S002 freezes the first 100+ product census; S003 extracts ontology from the 102-product corpus; S005 freezes the 200-product saturation probe; S006 reconstructs category history and corrects several novelty assumptions. S004 remains reserved for hands-on ontology verification.
- `02 Feature Taxonomy` contains the S003 ontology with 39 objects/actions/dimensions and product-level lexical evidence counts.
- `18 Category Archaeology` contains 24 milestones from 2007 through the 2026 agentic turn.
- The remaining comparison tabs stay intentionally sparse until hands-on verification begins.
