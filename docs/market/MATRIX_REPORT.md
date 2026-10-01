# Ortyo MCIF capability matrix

Checked: 2026-10-01  
DWC: MCIF/ORTYO.4 MATRIX

This is a decision projection over the canonical market evidence. It is **not** an overall product score.

Legend: **P** present, **~** partial, **A** verified absent, **?** unknown / not established by current research. External unknowns must not be read as absence.

## Primary decision matrix

| Capability | Ortyo | Webhook.site | Hookdeck | Webhook Relay | Svix | Beeceptor | webhooks.cc | Hooklistener | Webhooker | Hook0 | RequestBin |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Instant public endpoint | P | P | P | P | P | P | P | P | P | P | P |
| No-signup ephemeral start | P | P | P | ? | ? | P | P | P | ? | ? | ? |
| Persistent endpoint | ~ | ~ | P | P | P | ~ | P | P | P | P | P |
| Request inspection | P* | P | P | P | P | P | P | P | P | P | P |
| Request history | P | P | P | P | P | P | P | P | P | P | P |
| Search/filter | ~ | P | P | ? | ? | ? | P | P | P | ? | ~ |
| Configurable response | ~ | P | ? | ? | P | P | P | P | ? | ? | ~ |
| Failure/latency simulation | ~ | ~ | ? | ? | P | P | ~ | ~ | ? | ? | ? |
| Replay | P* | P | P | P | ~ | ? | P | P | P | P | P |
| Local forwarding | P | P | P | P | P | P | P | P | P | ? | ? |
| Provider templates | A | ? | ? | ? | ? | ? | P | ? | ? | ? | P |
| Signature verification | ? | ? | ? | ? | P | ? | P | P | ? | P | ? |
| API | P | P | P | P | P | P | P | P | P | P | P |
| CLI | P | P | P | P | P | ? | P | P | P | ? | ? |
| MCP/agent surface | P | ? | P | P | ? | ? | P | P | ? | P | P |
| Async wait | P | ? | ? | P | P | ? | P | ~ | ? | ? | ? |
| Deterministic CI | P | ? | ~ | ~ | ~ | ~ | P | P | ? | ? | ~ |
| Structured request diff | ? | ? | ? | ? | ? | ? | P | P | ? | ? | ? |
| Self-host/local runtime | P | ? | ~ | ? | ? | ? | ? | ? | ? | P | ? |
| Team collaboration | ~ | P | P | P | P | P | P | P | P | P | P |
| Durable retries | ? | ? | P | P | P | ? | ? | ? | P | P | ? |
| DLQ | A | ? | ? | ? | ? | ? | ? | ? | P | ? | ? |
| API/service mocking | ~ | ~ | ? | ? | ~ | P | ~ | ~ | ? | ? | P |

`P*` means the capability exists in Ortyo but the current human-facing depth is materially below the primary-market reference bar.

## Important correction from this pass

The previous depth pass described Ortyo as occupying a distinct verification-first center. That remains a useful description of Ortyo's architecture, but it was too broad as a differentiation claim.

**webhooks.cc** already exposes isolated/ephemeral endpoints, bounded waits, expected request counts, structured request assertions, structured diffs, deterministic cleanup, provider templates/signing, and CI-focused workflows.

**Hooklistener** added Replayable Integration Cases with saved captured requests, response assertions, named suites, reusable replay targets, durable suite run reports, and REST/MCP execution.

Therefore **assertions, suites, and CI automation are no longer sufficient differentiation by themselves**.

## Sharpened Ortyo differentiation hypothesis

The remaining unusual combination in Ortyo is deeper **temporal and causal evidence semantics**:

```text
canonical Interaction identity
+ portable Recording/Replay
+ exact/ranged cardinality
+ durable observed ordering
+ explicit observation/settle windows
+ correlation/causation/idempotency context
+ persisted assertion evidence
+ deterministic behavior-vs-infrastructure outcome
```

The stronger question is:

> Can Ortyo make asynchronous multi-event integration behavior explainable and deterministic in ways that simpler request assertions and replay suites cannot?

That is narrower and more defensible, but still requires first-party demand evidence.

## Minimum gap to primary-cohort completeness

The current implementation already covers substantial machine-facing functionality. The shortest path to a credible primary product is mostly **productization depth**, not new infrastructure.

1. **Human inspector depth - MUST.** Request list/detail, structured query/headers/body, JSON/raw views, timestamps/sequence, copy body/headers/cURL, and selected-request actions.
2. **Search/filter - MUST.** Useful human search/filter over retained interactions.
3. **Configurable Hook response - MUST.** Narrow response policy for status, headers, body, and delay.
4. **Replay ergonomics - MUST.** Put replay directly on captured requests and preserve replay-result evidence.
5. **Provider template + signing adapter - SHOULD/NEXT.** Start with an adapter contract and a small high-value provider set.
6. **Structured request/attempt diff - SHOULD/NEXT.** Feed both human debugging and CI explanation.
7. **Persistent/claimed Hook product flow - SHOULD for pilot.** Make anonymous-first/claim-later feel like a stable endpoint across sessions.
8. **Collaboration depth - SHOULD for pilot.** Turn workspace/security primitives into coherent shared evidence and team flows.

## What is already sufficiently covered for the primary development wedge

No immediate foundational build is required for public Hook creation, anonymous start, canonical capture/history, API, CLI, MCP, local forwarding/exposure, Recording/Replay, bounded async observation, Contracts/Scenarios, cardinality, ordering, correlation/idempotency context, deterministic CI semantics, or the local/self-hosted core.

This is why generic compute, tunneling, or orchestration work should not outrank the eight gaps above.

## Expansion interpretation

Delivery-first products remain much deeper in retries and DLQ. Beeceptor remains much deeper in service virtualization. Those are valid vectors, but the matrix does not justify copying them now.

Use the existing vector triggers:

- stable Hooks becoming pilots -> revisit reliable delivery
- repeated response/fault requirements -> deepen virtualization
- repeated provider pain -> expand adapters
- repeated multi-event CI usage -> deepen temporal/causal verification

## Research debt

The matrix is intentionally incomplete where evidence is incomplete. Reduce unknowns only for cells that could change a current decision. Do **not** fill every cell merely to make the table look complete.
