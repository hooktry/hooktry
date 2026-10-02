# Hooktry MCIF dogfood proof

Checked: 2026-10-01  
DWC: MCIF/Hooktry.14 DOGFOOD-PROOF

## Purpose

FIRST-PARTY-PROOF added the instrumentation and two temporal recipes.

DOGFOOD-PROOF verifies that those probes actually distinguish buggy from corrected behavior through the public CLI/Exposure path.

This is **mechanism evidence**, not customer-demand evidence.

## Acceptance flow

CI runs:

```sh
cargo test --locked --test first_party_dogfood_cli -- --nocapture
```

The test starts:

1. a real local target HTTP service
2. a real Hooktry HTTP boundary
3. the real `hooktry scenario run` binary
4. child processes that send traffic through `HOOKTRY_EXPOSURE_URL`

It materializes the repository-owned recipes with the test target's ephemeral port.

## Duplicate/idempotency proof

Recipe:

```text
examples/scenarios/duplicate-idempotency.json
```

Buggy flow sends the same logical interaction twice with the same correlation/idempotency context.

Expected result:

```text
child command: PASS
ScenarioOutcome: FAIL
CLI exit: 1
```

Corrected flow sends it once.

Expected result:

```text
child command: PASS
ScenarioOutcome: PASS
CLI exit: 0
```

This proves the failure is attributed to observed integration behavior rather than to the child process.

## Ordering proof

Recipe:

```text
examples/scenarios/out-of-order.json
```

Buggy flow sends:

```text
subscription -> customer
```

while the Scenario declares:

```text
customer -> subscription
```

Expected result:

```text
child command: PASS
ScenarioOutcome: FAIL
CLI exit: 1
```

Corrected flow sends the declared order and passes with exit 0.

## Usage evidence acceptance

All four runs append to one explicit local JSONL sink.

The acceptance asserts:

- exactly four usage events
- four distinct telemetry event IDs
- buggy duplicate event is `passed=false` with `duplicate_guard=true`
- fixed duplicate event is `passed=true`
- buggy order event is `passed=false` with `ordering=true`
- fixed order event is `passed=true`
- child commands remain successful in both negative cases
- usage evidence does not contain webhook paths or correlation/idempotency values

This proves the first-party evidence loop can distinguish:

```text
product orchestration failure
vs
child command failure
vs
behavioral temporal failure
```

for the thin probes.

## What this does not prove

This test is intentionally synthetic.

It does **not** prove:

- external developer demand
- willingness to pay
- unique-user retention
- that a naturally occurring bug was discovered
- that users prefer Contract/Scenario syntax
- that richer temporal DSL should be built

No MCIF demand signal is added for this test.

## Decision impact

Keep:

```yaml
horizon: validate
decision: differentiation
next_evidence: first_party_usage
```

The mechanism is now sufficiently proven for the two selected failure classes.

The next meaningful evidence must come from a live integration workflow where the probe is used because the integration needs testing, not because the test was constructed solely to exercise Hooktry.

## Promotion trigger

A stronger first-party signal can be recorded when at least one of these occurs:

- Hooktry dogfoods a real webhook integration and the probe catches an unintended duplicate/order bug
- another project adopts a temporal recipe and keeps rerunning it
- a developer modifies the thin recipe because they need richer cardinality/order/settle/context behavior
- direct feedback asks for a deeper primitive after using the thin proof

Until then, more synthetic variants should not increase roadmap priority.
