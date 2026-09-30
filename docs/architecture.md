# Architecture

## Product boundary

ORTYO is a programmable boundary between an application and its external dependencies.

It is intentionally not a general observability platform. The core must work without OpenTelemetry or application instrumentation.

## Canonical primitives

### Boundary
An executable edge that can observe, proxy, emulate, or replay an external dependency.

### Session
A runtime scope that correlates interactions produced by one scenario, test, developer action, or agent task.

### Interaction
Canonical evidence of an application crossing a boundary. Every interaction carries protocol, direction, origin, operation, timing, input evidence, and output evidence.

Origins are: observed, proxied, emulated, replayed, generated.

### Recording
A portable collection of captured interactions suitable for deterministic replay.

### Contract
Selectors and assertions over interactions and their temporal relationships.

## First vertical slice

    ortyo
      |
      +-- HTTP boundary :7777
      |      +-- capture request
      |      +-- produce response
      |      +-- record timing
      |      v
      +-- InteractionStore
             |
             v
      GET /_ortyo/interactions

This proves the canonical evidence path before persistence, UI, replay, assertions, MCP, SMTP, feature flags, or other adapters.

## Extension rule

An adapter belongs in ORTYO when it meaningfully supports the same lifecycle: Observe -> Control -> Replay -> Assert.

Candidate semantic boundaries include HTTP, SMTP, gRPC, feature flags via OpenFeature/provider hooks, queues, object storage, identity, notifications, and LLM calls.

## OpenTelemetry

OTEL is optional enrichment. Trace/span correlation may be attached to a Session or Interaction, but telemetry is not the source of truth for boundary evidence.
