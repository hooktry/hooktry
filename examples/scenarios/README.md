# Scenario proof recipes

These manifests exercise the two temporal failure classes currently selected for Ortyo first-party validation.

## Duplicate / idempotency

`duplicate-idempotency.json` expects one matching logical side effect for a fixed correlation/idempotency context and keeps the observation open for a quiet period.

A child test can intentionally expose a duplicate bug with:

```sh
curl -sS -X POST "$ORTYO_EXPOSURE_URL/webhook" \
  -H 'x-correlation-id: checkout-demo' \
  -H 'idempotency-key: payment-demo' \
  -d '{}'

curl -sS -X POST "$ORTYO_EXPOSURE_URL/webhook" \
  -H 'x-correlation-id: checkout-demo' \
  -H 'idempotency-key: payment-demo' \
  -d '{}'
```

Run it around the application test:

```sh
ortyo scenario run examples/scenarios/duplicate-idempotency.json -- ./your-test-command
```

The scenario passes only when exactly one matching interaction is observed through the settle window.

## Out-of-order behavior

`out-of-order.json` declares that `POST /customer` must be observed before `POST /subscription`.

A reversed child flow:

```sh
curl -sS -X POST "$ORTYO_EXPOSURE_URL/subscription" -d '{}'
curl -sS -X POST "$ORTYO_EXPOSURE_URL/customer" -d '{}'
```

causes the scenario outcome to fail even when both request contracts pass.

Run it with:

```sh
ortyo scenario run examples/scenarios/out-of-order.json -- ./your-test-command
```

## First-party proof telemetry

Scenario usage evidence is disabled unless a sink is explicitly configured.

Local dogfood JSONL:

```sh
ORTYO_USAGE_LOG=.ortyo/usage.jsonl \
  ortyo scenario run examples/scenarios/duplicate-idempotency.json -- ./your-test-command
```

Optional HTTP sink:

```sh
ORTYO_USAGE_ENDPOINT=https://your-ortyo-host/api/v1/usage-events \
ORTYO_USAGE_TOKEN=... \
  ortyo scenario run examples/scenarios/out-of-order.json -- ./your-test-command
```

The event contains only feature/result aggregates. It does not contain scenario/run/exposure IDs, names, ports, URLs, child commands, headers, request/response bodies, or correlation/idempotency values.
