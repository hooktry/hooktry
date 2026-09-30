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

`ortyo mcp` starts a stdio MCP server backed by the same HTTP API. The first tools are `interactions_list` and `contract_assert`, so agents consume the same canonical evidence and assertion semantics as the CLI.

## License

TBD before the first public release.
