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

The Worker exposes the same public Hook API and capability routes as the native implementation:

- `POST /api/v1/hooks`
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

## Production deployment

Production deployment is intentionally explicit and runs through the GitHub Actions **Deploy Cloudflare** workflow using the GitHub `production` environment.

This is an architectural boundary, not a temporary convenience: GitHub Actions is the canonical ORTYO deployment orchestrator, while Cloudflare is a deployment provider. Provider-native CI/CD may be added later for narrow provider-specific value, but must not become a second independent production deployment authority. See `docs/rfc/deployment-orchestration.md`.

Ordinary deployment is deliberately **not** infrastructure bootstrap. It uses the checked-in `wrangler.production.jsonc`, which identifies the already-provisioned ORTYO Worker, D1 database, and R2 bucket. Resource identifiers are deployment configuration, not credentials.

The ordinary workflow:

1. validates the production environment credentials
2. verifies the Cloudflare adapter in the Workers runtime
3. applies D1 migrations to the existing `ortyo-cloudflare` database
4. deploys the existing Worker / Durable Object configuration with `ORTYO_RELEASE_SHA=<Git SHA>`
5. waits for `/healthz` to report that exact revision consistently before live traffic acceptance
6. runs a live workers.dev acceptance:
   `create -> view -> hook -> claim -> same hook -> single-use claim`
7. removes the smoke Interaction bodies and D1 rows

The convergence gate is deliberate. It verifies release provenance and avoids starting realtime acceptance while a newly deployed Worker/Durable Object revision is still converging across the provider edge. It is not a retry of failed product behavior.

The GitHub `production` environment contains:

- secret `CLOUDFLARE_API_TOKEN` - long-lived deployment token
- secret `ORTYO_CLAIM_INTERNAL_TOKEN` - runtime claim authority used by acceptance
- secret `GITHUB_CLIENT_SECRET` - GitHub OAuth confidential client credential
- variable `CLOUDFLARE_ACCOUNT_ID` - non-secret Cloudflare account identifier
- variable `GITHUB_CLIENT_ID` - non-secret GitHub OAuth client identifier

`GITHUB_CLIENT_SECRET` is installed into the Worker by the separate manual **Configure Cloudflare Auth** workflow. Ordinary deploys pass `GITHUB_CLIENT_ID` as a Worker variable and never rewrite the OAuth secret.

The long-lived Cloudflare deployment token should use **Editor**, not Admin, and should be scoped to the existing ORTYO resources wherever Cloudflare offers resource scope:

- Worker `ortyo-cloudflare`
- D1 database `ortyo-cloudflare`
- R2 bucket `ortyo-payloads`

It must not have Account API Token provisioning authority.

### Bootstrap authority

Creating provider resources is a separate, exceptional operation.

The manual **Bootstrap Cloudflare** workflow uses `CLOUDFLARE_BOOTSTRAP_API_TOKEN`, not the ordinary deployment token. A bootstrap token may temporarily hold Admin authority needed to create D1/R2/Worker resources. It should be short-lived and removed from the GitHub environment after bootstrap.

The bootstrap workflow:

1. finds or creates `ortyo-cloudflare` D1 in Eastern Europe
2. finds or creates `ortyo-payloads` R2 in Eastern Europe
3. generates an untracked Wrangler config for the discovered resource ids
4. applies migrations and deploys the Worker / Durable Object
5. installs `CLAIM_INTERNAL_TOKEN`
6. runs the same production acceptance
7. verifies that the checked-in production config still points to the bootstrapped D1 id

This separation prevents ordinary CI/CD from retaining resource-creation/deletion authority.

The initial acceptance endpoint uses the account Workers subdomain. Custom ORTYO domains are a separate networking slice after the runtime proof is green.

## GitHub authentication

AUTH1 adds a managed-cloud browser identity flow:

~~~text
GET  /api/v1/auth/github/start
GET  /api/v1/auth/github/callback
GET  /api/v1/session
POST /api/v1/logout
POST /claim/cl_<capability>
~~~

The GitHub OAuth access token is ephemeral and is discarded immediately after `GET https://api.github.com/user` returns the stable GitHub user id and current profile metadata.

OAuth state and Ortyo session values are bearer secrets in the browser but only SHA-256 digests are stored in D1. The authorization-code exchange uses PKCE.

`/healthz` exposes only whether both GitHub OAuth bindings are configured. Production acceptance requires that boolean to be true after AUTH1.

See `docs/rfc/github-auth-claim.md`.

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
