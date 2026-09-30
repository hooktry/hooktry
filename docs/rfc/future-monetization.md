# Future monetization: Ortyo Cloud

Status: exploratory product thesis, not a committed pricing plan  
DWC: FUTURE/MONETIZATION.1 MONETIZATION - monetize reliance, not core capability

## Context

Ortyo Cloud should let a developer understand and use the product meaningfully before paying.

The boundary we want to explore is not:

> pay to unlock the interesting protocol primitives.

It is:

> use the core product for free; pay when you start relying on Ortyo as durable infrastructure.

That suggests a monetization model centered on:

- durability
- continuity
- history
- collaboration
- scale

The product experience that motivated this thesis is simple: an ephemeral or time-limited service can be sufficient while experimenting, but once a user depends on stable state, long-lived identity, history, and recovery guarantees, continuity itself becomes the thing worth paying for.

## Product principle

Ortyo should avoid artificial degradation of correctness or security in the free tier.

The following should remain part of the trustworthy core product:

- encryption
- SecretRef safety
- destination/origin binding
- SSRF defenses
- redaction
- revocation
- exact-action approval semantics
- deterministic evidence contracts
- the core Observe -> Control -> Replay -> Assert model
- local / self-hosted OSS capability

Paid Cloud should primarily sell operational guarantees around those primitives rather than a safer version of them.

A useful shorthand:

```text
Security          free
Correctness       free
Core protocol     free
OSS/local runtime free

Durability        paid
Retention         paid
Convenience       paid
Scale             paid
Collaboration     paid
Managed compute   paid / usage-based
```

## Candidate free-to-paid boundaries

These are product hypotheses, not implementation commitments.

| Capability | Possible Free behavior | Possible paid behavior | Why it may monetize naturally |
| --- | --- | --- | --- |
| Stable Exposure URL | temporary or inactivity-expiring reservation | reserved indefinitely while subscribed | changing a webhook URL after it is wired into providers is operationally expensive |
| Hosted durable state | limited lifetime / inactivity retention | durable workspace state | production users need restart-safe continuity |
| Evidence retention | short window, e.g. 1-3 days | 30-90+ days | debugging, audit, replay, and agent context gain value over time |
| Replay history | bounded recent interactions | full retention window | regression/debugging becomes materially more useful |
| Secrets | small quota | larger quota | real integrations quickly need multiple provider credentials |
| Approval inbox | core manual flow | durable long-lived handoff and richer notification paths | human-in-the-loop becomes useful when users can leave and come back |
| Notifications | none or basic | Telegram/email/webhook/etc. | converts durable approvals into practical async workflows |
| Active Exposures | one or a small number | larger concurrent limit | serious projects naturally need multiple boundaries |
| Executions | small monthly quota | larger included quota | maps to real platform cost and usage |
| Private access | basic/public development modes | stronger private/token-scoped hosted access | production integrations need stricter ingress boundaries |
| Audit/export | basic local JSON | retained export / NDJSON / audit packages | useful for debugging, compliance, and external analysis |
| Custom domains | unavailable | paid or add-on | recognizable convenience feature that does not weaken the free core |
| Team/RBAC | single operator | members, roles, approvers, executor separation | collaboration has a separate willingness to pay |

## Strong candidate: Durable Cloud

Instead of making one isolated feature the first paywall, consider packaging the value as continuity.

Working concept:

> **Ortyo Durable Cloud**  
> Keep your endpoints, state, approvals, secrets, and evidence alive.

Illustrative only:

```text
Ortyo Cloud Free
$0

1 workspace
1 active Exposure
small secret quota
small execution quota
short evidence retention
manual approvals
1 concurrent runtime
temporary/inactivity-limited hosted reservations
```

```text
Ortyo Cloud
~$3-6/month hypothesis

1 workspace
multiple active Exposures
larger secret quota
larger execution quota
30-day-ish evidence retention
stable/reserved Exposure URLs
durable approval inbox
notifications
replay history
higher concurrency
```

Potential later team tier:

```text
Ortyo Team
pricing TBD

members
RBAC
separate approver/executor identities
longer retention
audit export
private Exposures
custom domains
```

The actual limits and prices must be derived from usage and cost data. The numbers above are placeholders for product exploration.

## Why stable Exposures may be a strong conversion point

A free user should be able to create an Exposure and prove an integration works.

The paid moment can begin when that Exposure becomes infrastructure.

Example:

```text
experiment
  -> create hosted Exposure
  -> wire it into GitHub / Stripe / Render / another provider
  -> integration works
  -> user starts depending on the URL remaining stable
  -> continuity has real value
```

This is a better conversion boundary than hiding the underlying Exposure primitive itself.

If free hosted URLs ever expire, Ortyo should avoid hostile surprise deletion. A production design should consider:

- explicit inactivity policy
- clear expiry timestamps
- advance notifications
- grace periods
- export/migration paths
- no silent identity reuse

## Why evidence retention may be a strong conversion point

Ortyo's model is inherently temporal.

Interactions, executions, approvals, recordings, assertions, and outcomes become more valuable when retained.

Possible shape:

```text
Free      short evidence window
Cloud     30 days
Team      90+ days or configurable
```

This preserves the complete product loop for free while monetizing history rather than correctness.

## Why durable human-in-the-loop may be a strong conversion point

CONTROL1/CONTROL2 introduce a useful distinction between immediate approval and durable handoff.

Free can support:

```text
agent asks
  -> operator is present
  -> approve/deny
  -> continue
```

Managed paid Cloud can make this operationally durable:

```text
agent asks
  -> approval persists
  -> operator leaves
  -> notification arrives
  -> operator returns later
  -> approve/deny
  -> execution continues
  -> durable evidence remains queryable
```

The monetizable value is not the Approve button.

It is:

> Ortyo keeps holding the unfinished handoff safely while nobody is watching.

This also gives future Telegram, Slack, email, or webhook notifications a clean architecture: notifications point to durable approval state instead of becoming the workflow state themselves.

## Compute must be priced separately from cheap control-plane state

If Ortyo later runs browsers, sandboxes, computers, VMs, or other expensive execution environments, a flat low-cost unlimited plan is unlikely to be sustainable.

Prefer a model like:

```text
small platform subscription
+
included modest compute allowance
+
usage-based compute beyond that
```

HTTP relay, Postgres records, approval state, and small evidence payloads have a different cost profile from a browser or VM running for tens of minutes.

Do not force those two cost models into one unlimited low-priced tier.

## OSS / self-hosted relationship

The Cloud model should not require weakening OSS.

A useful long-term split could be:

```text
OSS / local
  -> powerful primitives
  -> user owns runtime, storage, uptime, backups, ingress, notifications

Ortyo Cloud
  -> managed continuity
  -> hosted durable state
  -> stable ingress
  -> retained evidence
  -> managed notifications
  -> collaboration
  -> scale
```

This makes Cloud payment understandable: users pay Ortyo to operate the boring persistent parts reliably, not to regain functionality intentionally removed from OSS.

## Anti-patterns to avoid

Do not monetize by weakening safety.

Avoid:

- charging for encryption
- charging for SSRF protection
- charging for redaction
- charging for revocation
- making free exact-action approval less strict
- intentionally losing data without clear retention semantics
- making free impossible to evaluate end-to-end
- unlimited expensive managed compute in a low flat tier
- turning notifications into a second source of workflow truth
- introducing a generic workflow/task engine only to justify a higher tier

## Cost-aligned axes to measure before final pricing

Before committing to tiers, measure:

- stored evidence bytes per workspace
- retained interaction count
- active Exposure hours
- relay request volume
- execution count and duration
- secret count
- approval lifetime
- notification volume
- concurrent runtimes
- database/storage cost
- egress
- future sandbox/browser/VM compute cost

Pricing should follow real cost curves and observed willingness to pay, not the placeholder numbers in this RFC.

## Open questions

1. Should free hosted Exposures expire after inactivity, after a fixed lifetime, or only lose reservation guarantees?
2. Is stable Exposure identity enough to anchor the first paid tier, or should paid begin only when state/evidence retention is extended?
3. What minimum retention window makes Free useful without creating disproportionate storage cost?
4. Should durable approval inbox itself remain free while notifications are paid?
5. Should custom domains be included in the base paid tier or treated as an add-on?
6. At what point do collaboration and RBAC justify a distinct Team tier?
7. Which managed compute providers can be passed through transparently enough for usage-based billing?
8. What export guarantees should users have before any hosted data ages out?
9. Should billing follow workspace, user, project, or organization boundaries?
10. Which paid guarantees can Ortyo prove automatically in dogfood the same way it proves runtime continuity today?

## Current conclusion

Do not lock pricing yet.

Preserve this as the current thesis:

> **Ortyo should monetize reliance, not curiosity.**

A user should be able to discover the full core loop for free. Payment should become natural when they want Ortyo Cloud to remain available, remember state, retain proof, hold human handoffs, collaborate, and scale.
