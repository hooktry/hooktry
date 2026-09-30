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
- **Scenario** - a reusable setup + evidence + replay + assertion definition that produces a persisted outcome.

ORTYO is not an observability backend. OpenTelemetry may enrich ORTYO evidence, but ORTYO does not require application instrumentation.

## CLI

Run the local boundary with `ortyo` or `ortyo serve`. The CLI talks to the same HTTP API used by other clients:

```sh
ortyo expose 3000
ortyo exposures
ortyo exposure-get <exposure-id>
ortyo exposure-revoke <exposure-id>
ortyo interactions
ortyo assert <contract-id> <interaction-id>
ortyo --base-url http://127.0.0.1:7777 interactions
```

Successful command output is structured JSON, so the same surface is suitable for scripts and agent tooling. `ortyo expose 3000` creates a private local exposure and returns `exposure_id`, `url`, `access`, `mode`, `target_port`, and a fail-closed `verified` result.

### MCP

`ortyo mcp` starts a stdio MCP server backed by the same HTTP API. Agents can manage Exposure lifecycle, inspect canonical evidence, create/replay recordings, and create/assert Contracts without bypassing ORTYO's HTTP boundary.

### Scenario

A Scenario packages the repeatable part of that workflow. Create it once with an upstream port and one or more operation-bound Contracts, then run it repeatedly:

```text
scenario_create
    ↓
scenario_start → unique Exposure URL
    ↓
external traffic
    ↓
scenario_complete
    ↓
filtered evidence → Recording → Replay → Contract assertions
    ↓
persisted ScenarioOutcome + automatic Exposure revoke
```

Scenario completion is idempotent. Evidence is scoped to the run's unique Exposure, so previous replays or unrelated traffic cannot enter the run. A run with no matching evidence completes as an explicit failed outcome rather than manufacturing evidence.

Scenario definitions can live in the repository as portable JSON manifests:

```json
{
  "name": "payment webhook",
  "target": {"port": 3000},
  "contracts": [
    {
      "name": "payment accepted",
      "operation": "POST /webhook",
      "request": {"body": "{\"event\":\"payment.created\",\"amount\":4999}"},
      "response": {"status": 202}
    }
  ]
}
```

Use the manifest and persisted run lifecycle from scripts or CI:

```sh
ortyo scenario create examples/scenarios/payment-webhook.json
ortyo scenario start <scenario-id>
# trigger the application behavior that sends traffic to the returned Exposure URL
ortyo scenario complete <run-id>
ortyo scenario outcome <run-id>
```

`scenario complete` is a deterministic CI gate: exit `0` for a passing ScenarioOutcome, exit `1` for a behavioral mismatch or missing expected evidence, and exit `2` for CLI, transport, API, or response errors. `ortyo assert` uses the same `0/1/2` convention.


## License

TBD before the first public release.
