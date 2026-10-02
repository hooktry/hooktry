# Evidence context, correlation, causality, and enrichment

Status: foundational design / incremental adoption

Implementation status:
- CTX1: implemented — normalized allowlisted HTTP context and replay preservation.
- MATCH1: implemented — all-candidate Contract matching with `count`/`min`/`max` cardinality evidence.
- WINDOW1: implemented — Scenario-level `within_ms` + quiet `settle_ms`, event-driven by persisted interaction revisions.
- CONTEXT-MATCH1: implemented — Contracts can subset-match canonical normalized correlation context, and Scenario/WINDOW1 use the same matcher.
- ORDER1: implemented — optional `ordering: "declared"` over durable source-interaction persistence order with machine-readable violations.
- SEQ1: implemented — every stored Interaction exposes `observed_sequence`; UUIDv7 remains identity/time-locality while observed sequence is canonical local commit order.
- Per-expectation WAIT1/eventually, RETRY1, OTEL1: deferred.

## Summary

HOOKTRY's source of truth is boundary evidence: what actually crossed an external integration boundary.

As HOOKTRY grows from single interaction assertions into multi-interaction scenarios, it needs enough normalized context to relate evidence without becoming an observability backend, workflow engine, message broker, or application transaction coordinator.

The governing order is:

```text
reality
  ↓
raw boundary evidence
  ↓
normalized context
  ↓
correlation / causality / provenance
  ↓
assertions
  ↓
ScenarioOutcome
```

In short:

> Evidence first. Enrichment second. Assertions third.

Normalized context must never replace the captured request/response evidence that produced it.

## Why this is needed before richer matching

A Scenario can already assert a replayed interaction by operation and payload. The next obvious features are cardinality, ordering, waiting, and retries.

Those features become much more useful if HOOKTRY can distinguish:

- two unrelated requests from two retries of one logical operation,
- interactions that belong to the same business flow,
- an interaction that caused another interaction,
- a replayed interaction from the original evidence it reproduces,
- transport identifiers from application identifiers,
- observed wire data from optional observability enrichment.

Without that layer, `count = 2` is ambiguous. It may mean a duplicate side effect, two legitimate operations, or one retry.

## Non-goals

This RFC does not make HOOKTRY:

- an OpenTelemetry backend,
- a logging backend,
- a metrics backend,
- an application tracing SDK,
- an event bus,
- a workflow engine,
- a transactional outbox implementation,
- a distributed transaction coordinator.

HOOKTRY may consume identifiers or references from those systems when they improve evidence interpretation.

## Canonical evidence versus normalized context

Raw or near-raw request and response data remains part of `Interaction.request` and `Interaction.response`.

For HTTP this includes headers even when HOOKTRY also extracts normalized identifiers from them.

Example:

```text
HTTP header: x-request-id: req_123
           │
           ├── remains in Interaction.request.headers
           │
           └── normalized into Interaction.context.correlation.request_id
```

If normalization rules change later, the original evidence is still available.

## Interaction context

The first context model should be intentionally small and backwards compatible.

Conceptually:

```rust
Interaction {
    ...
    request,
    response,
    source_interaction_id,
    context: InteractionContext,
}

InteractionContext {
    correlation: CorrelationContext,
    attributes: Map<String, Value>,
}

CorrelationContext {
    trace_id: Option<String>,
    parent_span_id: Option<String>,
    request_id: Option<String>,
    correlation_id: Option<String>,
    causation_id: Option<String>,
    message_id: Option<String>,
    idempotency_key: Option<String>,
}
```

All context fields are optional. Empty context is valid.

`attributes` is the extension point for protocol/provider/runtime-specific metadata that does not deserve a permanent top-level field.

Examples:

```json
{
  "correlation": {
    "trace_id": "4bf92f3577b34da6a3ce929d0e0e4736",
    "parent_span_id": "00f067aa0ba902b7",
    "request_id": "req_123",
    "idempotency_key": "payment-42"
  },
  "attributes": {
    "stripe.event_id": "evt_123",
    "github.delivery_id": "delivery-456",
    "queue.job_id": "job_987"
  }
}
```

### Why not one `correlation_id`

Real systems may simultaneously expose:

- W3C trace identity,
- request ID,
- business correlation ID,
- causation ID,
- message/event ID,
- job ID,
- idempotency key,
- provider-specific delivery IDs.

Collapsing these into one string destroys semantics and makes matching provider-specific conventions difficult.

## HTTP context extraction: CTX1

CTX1 should normalize only deterministic identifiers available from already-captured HTTP headers.

Initial candidates:

| Semantic field | Header candidates |
| --- | --- |
| trace | `traceparent` |
| request ID | `x-request-id`, `request-id` |
| correlation ID | `x-correlation-id`, `correlation-id` |
| causation ID | `x-causation-id`, `causation-id` |
| message ID | `x-message-id`, `message-id` |
| idempotency key | `idempotency-key`, `x-idempotency-key` |

Extraction must be deterministic and conservative:

- preserve the raw header regardless of extraction,
- take the first valid supported representation according to documented precedence,
- never invent an identifier,
- invalid identifiers remain raw evidence but do not populate normalized fields,
- do not make context presence required for capture or assertion.

### W3C `traceparent`

HOOKTRY should parse the W3C `traceparent` format conservatively.

The incoming parent ID is not an HOOKTRY span ID. HOOKTRY has not created a tracing span merely by observing traffic.

Therefore CTX1 stores:

- `trace_id`
- `parent_span_id`

and does not synthesize `span_id`.

The raw `traceparent` header remains in request evidence.

## Correlation versus causation

Correlation means evidence belongs to the same logical flow:

```text
A ─┐
B ─┼─ same flow
C ─┘
```

Causation means one event led to another:

```text
A → B → C
```

They should not be conflated.

Headers such as `x-causation-id` may provide application-level causation identifiers. Later HOOKTRY may also add first-class evidence links.

## Provenance and links

HOOKTRY already has one provenance relation:

```text
replayed Interaction
    └── source_interaction_id → recorded source Interaction
```

That is useful and should remain stable for now.

A later generalized model may look like:

```json
{
  "links": [
    {"type": "replay_of", "interaction_id": "..."},
    {"type": "caused_by", "interaction_id": "..."}
  ]
}
```

Do not migrate to generic links in CTX1. First prove more than one real relation needs them.

Potential future relation types:

- `replay_of`
- `caused_by`
- `retry_of`
- `derived_from`

## Attempts and retries

Retries are a behavior HOOKTRY should eventually describe rather than hide.

Example:

```text
attempt 1 → timeout
attempt 2 → 503
attempt 3 → 200
```

A raw interaction count of three is not enough to say whether this is correct.

Future retry-aware assertions may use correlation/idempotency context to group attempts and express:

- total attempts,
- transient failures,
- final success/failure,
- retry delay/order,
- duplicate logical side effects.

Do not add an `attempt` field until HOOKTRY has a reliable grouping rule or an explicit source that provides attempt identity.

## Identity, content, execution, causality, provenance

As the evidence model evolves, treat these as distinct facets:

```text
identity
  operation
  endpoint
  message/event type
  idempotency identity

content
  headers
  body
  status/response

execution
  timestamps
  duration
  attempts/retries

causality
  trace/correlation/causation/message identity

provenance
  observed
  proxied
  replayed
  source evidence
```

This is a conceptual separation, not a requirement to create five new structs immediately.

## OpenTelemetry

OpenTelemetry is optional enrichment, not a dependency or source of truth.

Desired direction:

```text
raw boundary evidence ───────────────┐
                                    ↓
optional OTEL span/trace ── enrich → Interaction context
```

If `traceparent` is present on the wire, CTX1 can extract it without any OTEL SDK.

A later OTEL adapter may attach or reference information such as:

- `service.name`
- deployment/environment
- code namespace
- job name
- database/system attributes

HOOKTRY must remain useful when OTEL is absent or broken.

## Logs

HOOKTRY should not become Loki, Datadog, or another log store.

A later enrichment layer may attach references to relevant logs or runtime evidence using correlation/trace identity.

Logs should not silently become the authoritative input to Contract PASS/FAIL. Boundary evidence remains authoritative unless an assertion explicitly targets another evidence type in a future model.

## Metrics

Metrics are not core Scenario evidence today.

They may become useful for performance or resilience assertions later, but should remain a separate evidence/enrichment source rather than fields forced into every Interaction.

## Transactional outbox

HOOKTRY should not implement a transactional outbox.

The outbox is an application reliability pattern:

```text
database transaction
  ├── business state
  └── outbox row
        ↓ commit
publisher / worker
        ↓
external side effect
```

HOOKTRY is, however, well positioned to test the externally observable behavior that the pattern is intended to guarantee.

Example future Scenario:

```text
one business operation
  ↓
worker retry
  ↓
external payment call
  ↓
same idempotency context
  ↓
one logical external side effect
```

This makes correlation, idempotency identity, cardinality, and retry-aware assertions complementary features.

HOOKTRY may later gain adapters that observe an outbox table or message broker, but those are evidence sources, not an outbox implementation.

## Metadata and attributes

Provider-specific or runtime-specific identifiers should usually enter `InteractionContext.attributes`, not expand the core schema.

Candidate examples:

- `stripe.event_id`
- `stripe.request_id`
- `github.delivery_id`
- `cloudevents.id`
- `queue.job_id`
- `queue.message_id`
- `runtime.name`
- `service.name`

Naming should be namespaced when semantics are provider/protocol specific.

Attributes should be deterministic JSON-compatible values.

## Sensitive metadata and redaction

Context normalization creates an additional indexed/queryable view of data, so it must not accidentally copy secrets merely because a header exists.

CTX1 must use an allowlist of known identifier headers. It must not copy all headers into `attributes`.

Future configurable extraction must be paired with explicit redaction/classification rules.

Authentication material such as `authorization`, cookies, API keys, and bearer tokens must never be promoted into correlation context.

The raw evidence retention/redaction policy is a separate concern and should be designed explicitly before production hosted evidence storage expands.

## Scenario matching roadmap

After CTX1, richer Scenario matching can build on normalized context.

### MATCH1

Evaluate all candidate interactions rather than `find(first)` and support cardinality:

- `count`
- `min`
- `max`

Examples:

```text
expected count 1, matched 0 → FAIL missing
expected count 1, matched 1 → PASS
expected count 1, matched 2 → FAIL duplicate
min 1 max 3, matched 2       → PASS
```

Outcome evidence should include candidate/matched interaction IDs and expected/observed cardinality.

### ORDER1

Implemented as an optional Scenario-level policy:

```json
{
  "ordering": "declared",
  "contracts": [
    {"name": "customer", "operation": "POST /customers"},
    {"name": "subscription", "operation": "POST /subscriptions"},
    {"name": "email", "operation": "POST /emails"}
  ]
}
```

The declaration order defines the expected partial sequence of matching groups. Every match of an earlier expectation must be persisted before every match of each later expectation. Cardinality remains a separate concern, so two expectations may both satisfy their Contracts/counts while ORDER1 still fails.

ORDER1 is based on a durable monotonic persistence sequence, not `started_at`, UUID ordering, or wall-clock comparison. SQLite keeps that sequence in an internal `interaction_order` table. Existing local databases are backfilled once from SQLite insertion order; all new interactions persist their evidence row and order row atomically.

SEQ1 exposes that same value as `Interaction.observed_sequence`. The field is additive/optional in serialized legacy payloads, but every Interaction read from an HOOKTRY store is hydrated with its sequence. UUIDv7 remains the canonical globally unique Interaction identity and is useful for approximate time locality; it is not treated as the strict observation-order contract. Replays receive a new observed sequence because they are new evidence records, while `source_interaction_id` preserves lineage to the original evidence.

The outcome contains the observed source interaction IDs and explicit violating pairs. This is observable boundary order only; it does not claim application-level causality. Missing groups do not invent an order relation.

### WAIT1

Eventually/timeout semantics for asynchronous jobs and delayed external effects.

Example:

```json
{
  "operation": "POST /webhook",
  "eventually": {"within_ms": 5000}
}
```

### RETRY1

Retry-aware grouping and assertions after correlation semantics are proven.

### OTEL1

Optional trace/span enrichment when a real use case requires more than wire-level header extraction.

## Replay implications

Current replay reproduces canonical evidence and preserves `source_interaction_id`.

When context is added, replay must preserve the source Interaction context. Replay must not re-extract or reinterpret historical headers using current rules, because that would make replay semantics change when normalization code changes.

Actual execution replay is a separate future layer:

```text
recorded request
  ↓
real or emulated dependency
  ↓
new observed response
  ↓
compare against recorded/contract expectations
```

## Decisions for now

Implement now:

1. Backwards-compatible `InteractionContext`.
2. Canonical correlation fields:
   - `trace_id`
   - `parent_span_id`
   - `request_id`
   - `correlation_id`
   - `causation_id`
   - `message_id`
   - `idempotency_key`
3. Deterministic HTTP header extraction using an allowlist.
4. Conservative W3C `traceparent` parsing.
5. Empty `attributes` extension map.
6. Context preservation through Recording/Replay.
7. Tests proving raw headers remain untouched and replay preserves normalized context.

Explicitly defer:

- generic provenance links,
- generated HOOKTRY span IDs,
- OTEL SDK/collector dependency,
- log ingestion,
- metrics,
- automatic retry/attempt inference,
- provider-specific adapters,
- outbox implementation,
- generic Scenario step DSL.

## Resulting architecture

```text
                         optional future enrichment
                       ┌─ OTEL
                       ├─ logs references
                       ├─ runtime metadata
                       └─ provider adapters
                                │
                                ▼
application → HOOKTRY boundary → Interaction
                                │
                    ┌───────────┴───────────┐
                    ▼                       ▼
               raw evidence          normalized context
                                             │
                              correlation / causality
                                             │
                                             ▼
                                         Recording
                                             ↓
                                          Replay
                                             ↓
                                         Contracts
                                             ↓
                                         Scenario
                                             ↓
                                         CI decision
```

This keeps HOOKTRY focused on testable external behavior while leaving room for richer distributed-system reasoning later.
