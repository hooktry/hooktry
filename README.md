# ORTYO

A programmable integration boundary for observing, controlling, replaying, and verifying how software interacts with the outside world.

## Status

Early development. The first vertical slice focuses on an HTTP boundary and a canonical evidence model.

## Core model

- **Boundary** - an executable edge between the application and an external dependency.
- **Session** - a runtime grouping of related interactions.
- **Interaction** - canonical evidence of what crossed a boundary.
- **Recording** - portable captured evidence that can be replayed.
- **Contract** - assertions over observed interactions.

ORTYO is not an observability backend. OpenTelemetry may enrich ORTYO evidence, but ORTYO does not require application instrumentation.

## CLI

Run the local boundary with `ortyo` or `ortyo serve`. The CLI talks to the same HTTP API used by other clients:

```sh
ortyo interactions
ortyo assert <contract-id> <interaction-id>
ortyo --base-url http://127.0.0.1:7777 interactions
```

Successful command output is structured JSON, so the same surface is suitable for scripts and agent tooling.

### MCP

`ortyo mcp` starts a stdio MCP server backed by the same HTTP API. The production HTTP path is covered by an end-to-end proof: a local upstream is reached through an Exposure, the proxied interaction is recorded, replayed with provenance, matched by a Contract, and persisted as passing Assertion evidence.\n\nThe MCP toolset covers the evidence workflow: `interactions_list`, `recording_create`, `recording_replay`, `contract_create`, `contract_get`, `contract_assert`, and `assertion_get`. Agents consume the same canonical evidence, replay, and assertion semantics as the HTTP API.

## License

TBD before the first public release.
