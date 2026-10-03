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

## Workbook structure

| Tab | Purpose |
| --- | --- |
| 00 Market Map | Classify products and determine who belongs in the market |
| 01 Competitor Matrix | Compact side-by-side summary |
| 02 Feature Taxonomy | Canonical capability definitions |
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

`Company | Product | Product URL | Parent Company | Status | Category | Directness | Primary Job | Primary Cohort | Secondary Cohort | Delivery Model | Product Motion | Hooktry Overlap | Distinctive Angle | Source URL | Source Type | Observed At | Confidence | Needs Recheck | Research Status | Notes`

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

As of 2026-10-03 the map contains **35 products including Hooktry**.

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

As of 2026-10-03:

- Workbook structure is materialized.
- `00 Market Map` is seeded with the initial cohort.
- The Market Map is a native Google Sheets table with controlled dropdowns for classification/freshness fields.
- Remaining tabs exist as the agreed research skeleton and are intentionally not populated yet.
