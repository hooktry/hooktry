# ORTYO Cloudflare adapter

This app is the first managed-cloud implementation of the portable ORTYO runtime contracts.

It is an adapter, not the ORTYO domain model.

## Runtime mapping

| ORTYO capability | Cloudflare adapter |
| --- | --- |
| anonymous Exposure metadata / capability index | D1 |
| request bodies | R2 |
| per-Exposure serialization and live viewers | Durable Objects |
| live delivery | WebSocket Hibernation API |
| expiry cleanup | scheduled Worker |
| HTTP ingress | Worker routes |

The Worker exposes the same anonymous public routes as the native implementation:

- `POST /_ortyo/anonymous/exposures`
- `ANY /hook/hk_<capability>/*`
- `GET /view/vw_<capability>`
- `POST /claim/cl_<capability>`

## Local development

Install dependencies:

```sh
npm install
```

Create `.dev.vars`:

```text
CLAIM_INTERNAL_TOKEN=local-development-only
```

Run:

```sh
npm run dev
```

Tests run inside the Workers runtime with local D1, R2, and Durable Object bindings:

```sh
npm run check
```

## Production setup

The checked-in `wrangler.jsonc` uses a placeholder D1 database id. Before deployment:

1. create the D1 database and R2 bucket
2. replace the D1 `database_id` with the real id
3. ensure the R2 binding points at the production bucket
4. apply D1 migrations
5. set `CLAIM_INTERNAL_TOKEN` as a Worker secret
6. deploy the Worker

Example commands:

```sh
npx wrangler d1 migrations apply ortyo-cloudflare --remote
npx wrangler secret put CLAIM_INTERNAL_TOKEN
npx wrangler deploy
```

## Claim authority

CF1 is a data-plane slice, not the final account/login implementation.

The public claim capability is resolved by this adapter, but workspace authority currently arrives from the future authenticated control plane through:

- `Authorization: Bearer <CLAIM_INTERNAL_TOKEN>`
- `X-Ortyo-Workspace-Id: <uuid>`

This keeps login/team semantics out of the Cloudflare storage adapter. A later identity/control-plane slice will own the user-facing authentication flow and invoke the same atomic claim transition.

## Storage semantics

Capability plaintext is never persisted. D1 stores SHA-256 digests for hook, view, and claim capabilities.

Bodies are stored in R2 under an Exposure-scoped key. D1 stores interaction metadata and the R2 key.

The Durable Object does not become the source of truth for retained history. It coordinates per-Exposure ingestion and hibernating WebSocket viewers while D1/R2 remain durable storage.

Expired unclaimed resources are deleted by the scheduled cleanup handler.
