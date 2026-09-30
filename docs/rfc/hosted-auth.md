# AUTH1: Workspace Identity and Scoped API Credentials

Status: executable vertical slice  
Tracking: #72

AUTH1 separates hosted product identity from relay runtime authority.

## Authority model

ORTYO now has three distinct credentials:

```text
ORTYO_CONTROL_TOKEN
    bootstrap/admin authority only
        |
        +-- create Workspace
        +-- issue Workspace API credential

ORTYO_TOKEN
    workspace API authority
        |
        +-- exposures:create
        +-- exposures:read
        +-- exposures:revoke

runtime capability
    one Exposure only
        |
        +-- register runtime
        +-- self-revoke runtime
```

The master control token is no longer accepted by the hosted Exposure API.

## Workspace

A Workspace is the ownership boundary for hosted resources:

```json
{
  "id": "...",
  "slug": "acme",
  "status": "active"
}
```

Every new hosted Exposure is persisted with a `workspace_id`. Existing pre-AUTH1 rows remain readable by the runtime path but are not visible through workspace-scoped management APIs until explicitly migrated.

## Bootstrap

The bootstrap/admin API remains protected by `ORTYO_CONTROL_TOKEN`:

```http
POST /_ortyo/admin/workspaces
Authorization: Bearer <control-token>

{"slug":"acme"}
```

Then issue a credential:

```http
POST /_ortyo/admin/workspaces/<workspace-id>/credentials
Authorization: Bearer <control-token>

{
  "name": "developer-cli",
  "scopes": [
    "exposures:create",
    "exposures:read",
    "exposures:revoke"
  ]
}
```

The raw `ortyo_...` API token is returned only in the issuance response. Only its SHA-256 digest is persisted.

## Hosted Exposure API

User-facing requests use `ORTYO_TOKEN`:

```http
POST /_ortyo/hosted/exposures
GET  /_ortyo/hosted/exposures
GET  /_ortyo/hosted/exposures/<id>
DELETE /_ortyo/hosted/exposures/<id>

Authorization: Bearer <workspace-api-token>
```

Scope requirements:

- POST: `exposures:create`
- collection/item GET: `exposures:read`
- DELETE: `exposures:revoke`

A token never sees or revokes another Workspace's Exposure. Cross-workspace item lookup returns `404` so resource existence is not disclosed.

Missing/invalid credentials return structured `401` responses. Valid credentials missing a required scope return structured `403` responses.

## CLI

The public expose workflow now uses:

```sh
export ORTYO_TOKEN=ortyo_...
ortyo expose 3000 web --public
```

The CLI uses the workspace credential only for hosted provisioning. It still hands the local daemon only the narrow per-Exposure runtime capability.

## Persistence

Workspace identity and API credential digests use the same configured hosted persistence backend:

- PostgreSQL when `ORTYO_DATABASE_URL` is present
- SQLite otherwise

Raw API credentials are never persisted.

## Deliberate non-goals

AUTH1 does not add:

- user accounts
- OAuth/OIDC
- roles
- invitations
- billing
- arbitrary permission expressions

Those can layer over the stable Workspace + scoped credential model later.


## ONBOARD1: first workspace exchange

A fresh hosted installation can create its first Workspace without exposing the server-side control token:

```http
POST /_ortyo/bootstrap
Content-Type: application/json

{"slug":"serhii"}
```

The successful response is `201` and contains the Workspace plus one full-scope `initial-cli` credential. The raw `ortyo_...` token is returned in that response only; only its digest is persisted.

Bootstrap is atomic. Exactly one request can win, including under concurrent requests. After a Workspace exists, the endpoint permanently returns:

```json
{"error":{"code":"bootstrap_already_completed"}}
```

with HTTP `409 Conflict`.

This endpoint is intentionally only a fresh-install bridge. Subsequent workspace and credential creation remains an authenticated admin operation. A future OAuth/UI onboarding flow can replace the bootstrap transport without changing Workspace, scope, Exposure ownership, or runtime-capability semantics.
