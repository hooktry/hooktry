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
ORTYO_CONTROL_TOKEN='<secret>' ortyo expose 3000 web --public
ortyo exposures
ortyo exposure-get <exposure-id>
ortyo exposure-revoke <exposure-id>
ortyo interactions
ortyo assert <contract-id> <interaction-id>
ortyo --base-url http://127.0.0.1:7777 interactions
```

Successful command output is structured JSON, so the same surface is suitable for scripts and agent tooling. `ortyo expose 3000` creates a private local exposure. `ortyo expose 3000 web --public` provisions the hosted relay, attaches the exact Exposure to the local daemon, waits for authenticated WebSocket registration, and returns a public URL while `ortyo serve` keeps the tunnel alive.

### Hosted relay

Run provisioning, public ingress, and the authenticated WebSocket runtime tunnel on one HTTP listener:

```sh
ORTYO_BIND=0.0.0.0:8080 \
ORTYO_PUBLIC_BASE_URL=https://relay.example \
ORTYO_CONTROL_TOKEN='<secret>' \
ORTYO_HOSTED_DB_PATH=/var/lib/ortyo/hosted.db \
ortyo hosted
```

Deployment platforms may provide `PORT` instead of `ORTYO_BIND`. The hosted process exposes `/healthz` and `/_ortyo/health`. Production should set `ORTYO_PUBLIC_BASE_URL` to the externally reachable HTTPS origin; runtime URLs are then provisioned as `wss://`. `ORTYO_CONTROL_TOKEN` is required and protects hosted Exposure provisioning; runtime registration uses a separate short-lived per-Exposure capability. Hosted Exposure metadata and capability digests are persisted through `ORTYO_HOSTED_DB_PATH` so runtime authority can survive process restarts. The raw capability is never stored.

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

Scenario contracts default to exactly one matching interaction. Cardinality can be made explicit without changing the single-interaction Contract matcher:

```json
{
  "name": "payment side effect",
  "operation": "POST /webhook",
  "count": 1,
  "request": {"body": "{\"event\":\"payment.created\"}"}
}
```

Or define a range:

```json
{
  "name": "bounded retry behavior",
  "operation": "POST /webhook",
  "min": 1,
  "max": 3
}
```

ORTYO evaluates every replayed interaction with the same operation, persists assertion evidence for every candidate, and applies `count`/`min`/`max` to the subset that actually matches the Contract. Scenario outcome evidence includes candidate, matched, and assertion IDs, so duplicate calls and payload mismatches remain distinguishable.

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

For the common CI path, ORTYO can orchestrate the lifecycle around an explicit child command:

```sh
ortyo scenario run examples/scenarios/payment-webhook.json -- bundle exec rspec spec/integration/payment_webhook_spec.rb
```

`scenario run` creates the Scenario, starts a unique Exposure, executes the child directly (without an implicit shell), completes the run even when the child exits non-zero, and prints one structured `ScenarioRunReport` to stdout. Child stdout/stderr is forwarded to ORTYO stderr so stdout remains machine-readable.

The child receives:

```text
ORTYO_BASE_URL
ORTYO_SCENARIO_ID
ORTYO_SCENARIO_RUN_ID
ORTYO_EXPOSURE_ID
ORTYO_EXPOSURE_URL
```

The command exits `0` only when both the child command and ScenarioOutcome pass, `1` when either behavior or the child command fails, and `2` when ORTYO cannot orchestrate or complete the run. If shell syntax is needed, invoke a shell explicitly after `--`.


## License

TBD before the first public release.
