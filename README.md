# Hooktry

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

Hooktry is not an observability backend. OpenTelemetry may enrich Hooktry evidence, but Hooktry does not require application instrumentation.

## CLI

Run the local boundary with `hooktry` or `hooktry serve`. The CLI talks to the same HTTP API used by other clients:

```sh
hooktry expose 3000
HOOKTRY_TOKEN='hooktry_...' hooktry expose 3000 web --public
hooktry exposures
hooktry exposure-get <exposure-id>
hooktry exposure-revoke <exposure-id>
hooktry interactions
hooktry assert <contract-id> <interaction-id>
hooktry --base-url http://127.0.0.1:7777 interactions

# Hosted ask -> handoff -> approve -> act -> prove
HOOKTRY_TOKEN='hooktry_...' \
  hooktry --base-url https://relay.example approval create request.json
HOOKTRY_APPROVER_TOKEN='hooktry_...' \
  hooktry --base-url https://relay.example approval inbox
HOOKTRY_APPROVER_TOKEN='hooktry_...' \
  hooktry --base-url https://relay.example approval get <approval-id>
HOOKTRY_APPROVER_TOKEN='hooktry_...' \
  hooktry --base-url https://relay.example approval approve <approval-id>
HOOKTRY_TOKEN='hooktry_...' \
  hooktry --base-url https://relay.example approval execute <approval-id> request.json
HOOKTRY_TOKEN='hooktry_...' \
  hooktry --base-url https://relay.example execution get <execution-id>
```

Successful command output is structured JSON, so the same surface is suitable for scripts and agent tooling. `hooktry expose 3000` creates a private local exposure. `hooktry expose 3000 web --public` provisions the hosted relay, attaches the exact Exposure to the local daemon, waits for authenticated WebSocket registration, and returns a public URL while `hooktry serve` keeps the tunnel alive.

### Hosted relay

Run provisioning, public ingress, and the authenticated WebSocket runtime tunnel on one HTTP listener:

```sh
HOOKTRY_BIND=0.0.0.0:8080 \
HOOKTRY_PUBLIC_BASE_URL=https://relay.example \
HOOKTRY_CONTROL_TOKEN='<secret>' \
HOOKTRY_DATABASE_URL='postgresql://...' \
HOOKTRY_APPROVAL_WEBHOOK_URL='https://hooks.example/...' \
hooktry hosted
```

Deployment platforms may provide `PORT` instead of `HOOKTRY_BIND`. The hosted process exposes `/healthz` and `/_hooktry/health`. Production should set `HOOKTRY_PUBLIC_BASE_URL` to the externally reachable HTTPS origin; runtime URLs are then provisioned as `wss://`. When `HOOKTRY_APPROVAL_WEBHOOK_URL` is set together with `HOOKTRY_BOOTSTRAP_WORKSPACE`, Hooktry encrypts that URL in SecretStore and delivers durable pending-approval notifications through the CONTROL3 outbox. The raw webhook URL is not logged or returned. `HOOKTRY_CONTROL_TOKEN` is required only for hosted bootstrap/admin operations. User-facing hosted Exposure APIs use workspace-scoped `HOOKTRY_TOKEN` credentials with `exposures:create`, `exposures:read`, and `exposures:revoke` scopes. Runtime registration uses a separate short-lived per-Exposure capability. Hosted Exposure metadata and capability digests use PostgreSQL when `HOOKTRY_DATABASE_URL` is set. Otherwise Hooktry falls back to SQLite via `HOOKTRY_HOSTED_DB_PATH`. The raw capability is never stored.

Hosted secret encryption supports explicit master-key versions. `HOOKTRY_SECRETS_KEY` is the active 32-byte key encoded as 64 hexadecimal characters. `HOOKTRY_SECRETS_KEY_VERSION` defaults to `1`. During a rotation, keep older roots in `HOOKTRY_SECRETS_PREVIOUS_KEYS` as comma-separated `version:64hex` entries until retirement is proven:

```sh
HOOKTRY_SECRETS_KEY='<new-v2-key>' \
HOOKTRY_SECRETS_KEY_VERSION=2 \
HOOKTRY_SECRETS_PREVIOUS_KEYS='1:<old-v1-key>' \
hooktry key status

# Preview only. No ciphertext is changed.
hooktry key rewrap 1

# CAS-protected rewrite from v1 to the active key.
hooktry key rewrap 1 --apply

# Remove v1 from HOOKTRY_SECRETS_PREVIOUS_KEYS only when this says safe_to_retire=true.
hooktry key retire-check 1
```

Key maintenance reads the same `HOOKTRY_DATABASE_URL` or `HOOKTRY_HOSTED_DB_PATH` as the hosted process and does not require `HOOKTRY_CONTROL_TOKEN`. Rewrap preserves secret identity and origin binding. It compares the original secret id, key version, and ciphertext before each write, so a concurrent secret rotation is skipped instead of overwritten. Historical keys remain required while encrypted secrets or pending/approved keyed approvals depend on them. Denied and consumed approvals are terminal and do not block retirement. Malformed keyed approval fingerprints fail closed. Key material is never included in command output.

### MCP

`hooktry mcp` starts a stdio MCP server backed by the same HTTP API. Agents can manage Exposure lifecycle, inspect canonical evidence, create/replay recordings, create/assert Contracts, and use the hosted approval/execution lifecycle without bypassing Hooktry's HTTP boundary.

For hosted CONTROL1 + EXEC5 tools, configure the MCP process with `HOOKTRY_TOKEN` for ask/act/query authority. Configure `HOOKTRY_APPROVER_TOKEN` only when that process is intentionally allowed to approve or deny. `approval_decide` never falls back to `HOOKTRY_TOKEN`. Credentials are environment configuration, not MCP tool arguments.

### Agent discovery

Both local and hosted Hooktry HTTP services publish agent-readable discovery surfaces:

- `/llms.txt` - concise product, interface, and safety index.
- `/llms-full.txt` - self-contained projection composed from `llms.txt`, this README, and the Agent Skill.
- `/skills/hooktry/SKILL.md` - operational agent workflow and sharp edges.

The repository keeps the canonical concise index at `llms.txt` and the skill at `skills/hooktry/SKILL.md`. MCP `tools/list` remains authoritative for the live tool names and JSON input schemas, so the prose surfaces do not duplicate a second tool catalog.

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

Hooktry evaluates every replayed interaction with the same operation, persists assertion evidence for every candidate, and applies `count`/`min`/`max` to the subset that actually matches the Contract. Scenario outcome evidence includes candidate, matched, and assertion IDs, so duplicate calls and payload mismatches remain distinguishable.

Contracts can also match the normalized correlation context extracted from boundary evidence:

```json
{
  "name": "checkout payment",
  "operation": "POST /payments",
  "context": {
    "correlation_id": "checkout-42",
    "idempotency_key": "payment-42"
  },
  "count": 1
}
```

Context matching is subset-based like request/response matching. Supported canonical fields are `trace_id`, `parent_span_id`, `request_id`, `correlation_id`, `causation_id`, `message_id`, and `idempotency_key`. Raw headers remain evidence; the Contract matches the normalized values. This lets two otherwise identical external calls be distinguished by their logical flow or idempotency identity without making OpenTelemetry mandatory.

Asynchronous integrations can define a Scenario-level observation window:

```json
{
  "observation": {
    "within_ms": 5000,
    "settle_ms": 500
  }
}
```

`within_ms` is the hard observation horizon after completion begins. `settle_ms` is a quiet period that permits early completion once all positive expectations currently pass. A zero `settle_ms` waits the full `within_ms`. Expectations that can pass with zero matches, such as `count: 0` or max-only constraints, always wait the full window because absence cannot be proven early. An irreversible cardinality overflow fails immediately.

Interaction arrival wakes Scenario completion through an in-process revision notification after persistence; SQLite remains the source of truth and no polling loop is used. The final `ScenarioOutcome` records both the observation policy and elapsed observation time.

Scenarios can also assert the observed order of their contract definitions:

```json
{
  "ordering": "declared",
  "contracts": [
    {"name": "customer", "operation": "POST /customers", "count": 1},
    {"name": "subscription", "operation": "POST /subscriptions", "count": 1},
    {"name": "email", "operation": "POST /emails", "count": 1}
  ]
}
```

`declared` means every matching source interaction for an earlier contract must be persisted before every matching source interaction for each later contract. Ordering is evaluated independently from cardinality: both contracts can individually pass and the Scenario can still fail because their observed order is reversed. Missing optional/zero-match groups do not create an order edge.

Hooktry does not use wall-clock timestamps or UUID ordering to decide order. Every persisted Interaction exposes `observed_sequence`: the durable local order in which this Hooktry evidence store committed it. The internal `interaction_order` table owns the monotonic sequence; existing databases are backfilled once from SQLite insertion order, and old payloads are hydrated with the sequence when read.

`Interaction.id` remains UUIDv7 and answers identity / approximate time locality. `observed_sequence` answers a different question: which interaction this Hooktry store committed first. Replayed interactions receive their own later observed sequence while preserving `source_interaction_id`. `ScenarioOutcome.order` records source interaction IDs and machine-readable violating contract/interaction pairs.

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
hooktry scenario create examples/scenarios/payment-webhook.json
hooktry scenario start <scenario-id>
# trigger the application behavior that sends traffic to the returned Exposure URL
hooktry scenario complete <run-id>
hooktry scenario outcome <run-id>
```

`scenario complete` is a deterministic CI gate: exit `0` for a passing ScenarioOutcome, exit `1` for a behavioral mismatch or missing expected evidence, and exit `2` for CLI, transport, API, or response errors. `hooktry assert` uses the same `0/1/2` convention.

For the common CI path, Hooktry can orchestrate the lifecycle around an explicit child command:

```sh
hooktry scenario run examples/scenarios/payment-webhook.json -- bundle exec rspec spec/integration/payment_webhook_spec.rb
```

`scenario run` creates the Scenario, starts a unique Exposure, executes the child directly (without an implicit shell), completes the run even when the child exits non-zero, and prints one structured `ScenarioRunReport` to stdout. Child stdout/stderr is forwarded to Hooktry stderr so stdout remains machine-readable.

The child receives:

```text
HOOKTRY_BASE_URL
HOOKTRY_SCENARIO_ID
HOOKTRY_SCENARIO_RUN_ID
HOOKTRY_EXPOSURE_ID
HOOKTRY_EXPOSURE_URL
```

The command exits `0` only when both the child command and ScenarioOutcome pass, `1` when either behavior or the child command fails, and `2` when Hooktry cannot orchestrate or complete the run. If shell syntax is needed, invoke a shell explicitly after `--`.

FIRST-PARTY-PROOF includes ready-made temporal recipes:

```sh
hooktry scenario run examples/scenarios/duplicate-idempotency.json -- ./your-test-command
hooktry scenario run examples/scenarios/out-of-order.json -- ./your-test-command
```

Usage evidence is off by default. For explicit local dogfood, set `HOOKTRY_USAGE_LOG` to an append-only JSONL path. An optional HTTP sink uses `HOOKTRY_USAGE_ENDPOINT` plus `HOOKTRY_USAGE_TOKEN`. The usage event contains only verification feature/result aggregates - never Scenario/run/exposure IDs, names, commands, URLs, headers, payloads, or correlation/idempotency values. See [examples/scenarios/README.md](examples/scenarios/README.md) and [docs/rfc/first-party-usage-evidence.md](docs/rfc/first-party-usage-evidence.md).


## First public release

The first public release is gated by the production acceptance checklist in [docs/ops/first-public-release-gates.md](docs/ops/first-public-release-gates.md). In particular, production must complete and prove the first real master-key rotation from v1 to v2 before the release/tag is created.

## License

TBD before the first public release.
