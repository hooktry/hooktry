# Ortyo MCIF depth report - primary webhook cohort

Checked: 2026-10-01
DWC: MCIF/ORTYO.2 DEPTH

This report compares where primary-cohort products go deep. It intentionally does not calculate an aggregate score.

## Profile summary

Labels are summaries of independently evidenced dimensions, not rankings. `unknown` means insufficient current evidence, not absence.

| Product | Capture / inspect | Sender response / faults | Local dev | Replay / provider testing | CI / agent | Deterministic verification | Reliable delivery |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **Ortyo** | present | basic | deep | present | deep | **deep** | basic / emerging |
| Webhook.site | deep | deep | deep | present | present | unknown | basic |
| Hookdeck | deep | present | deep | deep | present | basic | **deep** |
| Webhook Relay | deep | present | deep | deep | deep | present | **deep** |
| Svix / Play | present | present | present | deep | present | basic | **deep** |
| Beeceptor | deep | **deep** | present | present | deep | basic | not focus |
| webhooks.cc | deep | deep | deep | **deep** | **deep** | **deep** | basic |
| Hooklistener | deep | deep | deep | deep | present | present | basic |
| Webhooker | deep | basic | present | present | present | basic | **deep** |
| Hook0 / Play | present | basic | deep | present | deep | basic | **deep** |
| RequestBin.net | deep | present | basic | deep | present | unknown | basic |

## Market centers of gravity

The primary cohort currently clusters around three established centers.

**Inspection-first** products optimize `get URL -> receive -> inspect -> history/search -> replay/forward`. Webhook.site and RequestBin.net are strong references. This defines the minimum human-facing usability bar.

**Testing/virtualization-first** products extend the loop with controlled responses, failure/latency simulation, provider templates/signatures, CI automation, and dependency simulation. webhooks.cc and Beeceptor are strong references.

**Delivery-first** products extend the same entry point toward routing, transformations, retries, DLQ/failure recovery, signing, tenant controls, and operational visibility. Hookdeck, Webhook Relay, Svix, Hook0, and Webhooker are strong references.

Ortyo's current center is **verification-first**, but this pass shows that verification is no longer an uncontested category:

```text
Interaction
  -> Recording
  -> Replay
  -> Contract
  -> Scenario
  -> observation window
  -> cardinality
  -> ordering
  -> correlation / idempotency context
  -> persisted outcome
  -> deterministic CI result
```

webhooks.cc already exposes isolated endpoints, count/timeout-based capture, request assertions with structured diffs, and deterministic cleanup. Hooklistener exposes reusable replay cases, response assertions, suites, and durable run reports. Hookdeck, Webhook Relay, and Beeceptor also have meaningful CI/automation depth, but the current evidence supports only partial deterministic-CI semantics relative to Ortyo's stronger behavior-vs-infrastructure outcome contract.

Temporal-depth research narrows this again: webhooks.cc already supports count-bounded multi-event capture, timestamp-sorted request sequences, timeout horizons, reusable test flows, and structured request assertions; Hooklistener supports assertion-bearing replay cases, suites, bounded waits, and durable run reports. The remaining hypothesis is therefore **ranged cardinality plus proof of no extra events, durable observed-order assertions, hard horizon plus quiet/settle windows, normalized correlation/causation/idempotency context, and explainable behavior-vs-infrastructure outcomes.** This remains a hypothesis until supported by first-party demand evidence.

## Primary table-stakes gaps

### Human webhook inspector depth - MUST

The Cloudflare anonymous viewer is real and live, but currently intentionally minimal: a WebSocket stream appended as JSON to a `<pre>` block. It proves the flow, not market-complete inspection UX.

Required direction:

- request list plus selected request detail
- structured method/path/query/headers/body
- JSON pretty view with raw fallback
- copy body / headers / cURL
- arrival time and durable sequence
- replay action from the selected request
- useful empty/loading/disconnected states
- search/filter once retained history grows

This is part of the primary job, not optional polish.

### Configurable sender response - MUST

A webhook tester frequently needs to control sender-visible status, headers, body, and delay. Ortyo does not yet have evidence for a first-class configurable response policy on the anonymous Hook path.

Start narrow: Exposure/Hook response policy before a general-purpose mock engine.

### Replay ergonomics - MUST

The underlying Recording/Replay primitive exists. The missing depth is the product loop: `receive -> inspect -> replay to handler` should be immediate from the human surface as well as machine interfaces.

### Search/filter over retained evidence - MUST after inspector basics

Ortyo has structured evidence but not yet a mature human search/filter surface. This becomes table stakes once history is more than a handful of requests.

## High-value next capabilities

**Provider templates and signing - SHOULD.** webhooks.cc is particularly deep here and RequestBin.net also exposes provider samples. Build a generic adapter contract first, then seed a few common providers.

**Failure/latency simulation - SHOULD.** This is a natural extension of configurable responses and directly tests sender retry behavior. Avoid jumping immediately to full service virtualization.

**Attempt comparison / behavioral diff - SHOULD.** Ortyo's canonical evidence gives it a natural basis for semantic comparison of attempts or baseline/candidate behavior.

## Explicit non-priorities for now

**Full production delivery - WATCH.** The market validates this as a plausible depth vector, but it adds queues, durable retries, DLQ, rate controls, tenant isolation, delivery guarantees, and operational support. Deepen when users carry stable Ortyo endpoints into pilots and ask Ortyo to survive outages.

**Full API/service virtualization - WATCH.** First implement webhook-specific response/fault behavior. Expand only if users repeatedly need broader dependency simulation.

**Generic tunneling - ADJACENT/SUBSTRATE.** Ortyo needs reachability sufficient for the webhook lifecycle, not generic networking breadth.

**Generic managed compute - SUBSTRATE/OPTION.** Compute matters only when it improves reproducible integration behavior through Ortyo evidence.

## Strategic trigger map

```text
PRIMARY WEDGE WORKS
  +-> replay / automation / CI usage grows
  |      -> deepen deterministic verification
  +-> temporary hooks become long-lived pilots
  |      -> deepen reliable delivery
  +-> users need controlled sender behavior
  |      -> deepen webhook response/fault simulation
  |         -> maybe expand into API virtualization
  +-> users request SMTP/queues/gRPC/etc.
  |      -> evaluate multi-protocol expansion
  +-> agents become a major usage channel
         -> deepen approval-controlled actions

PRIMARY WEDGE DOES NOT WORK
  +-> do not jump to generic compute/tunnels
  +-> inspect direct-demand evidence
  +-> test the strongest adjacent job that still uses canonical Interaction/evidence primitives
```

## Near-term sequence

1. Mature the anonymous/live viewer into a useful webhook inspector.
2. Add narrow configurable response behavior.
3. Surface replay directly from captured requests.
4. Add history/search/filter ergonomics.
5. Add the first provider template + signing slice.
6. Deepen Scenario/CI/diff where Ortyo can distinguish itself.
