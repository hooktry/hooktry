# AUTH1 - GitHub sign-in and anonymous Hook claim

Status: executable vertical slice  
Checked: 2026-10-01

## Decision

The first managed-cloud human identity provider is GitHub.

AUTH1 exists to complete one product promise:

> create and use a Hook anonymously, then claim that exact Hook into a persistent workspace after it becomes valuable.

It is not a general IAM system and it does not grant repository access.

The flow is:

~~~text
anonymous Hook
    |
    | hook/view/claim capabilities already exist
    v
Claim
    |
    v
GitHub authorization code + PKCE
    |
    v
Hooktry browser session
    |
    v
personal workspace
    |
    v
atomic claim of the existing Exposure
~~~

The existing Hook capability and retained Interaction history remain unchanged.

## Why GitHub first

The primary cohort is product development, testing, and pilot workflows. GitHub is a common identity already present in that cohort and avoids asking for a password before the first useful Hooktry workflow.

AUTH1 uses a GitHub OAuth App only as an identity provider. It requests **no OAuth scopes**. GitHub's no-scope token is sufficient to read public profile identity, which gives Hooktry the stable GitHub user ID, current login, and avatar without repository, organization, email, or write access.

GitHub recommends GitHub Apps for repository integrations because they provide fine-grained permissions and short-lived credentials. That recommendation applies to future Hooktry repository/automation integrations. Those should use a separate GitHub App authority rather than expanding this sign-in credential.

## OAuth web flow

The managed Worker implements the server-side authorization-code flow.

~~~text
browser
  |
  | GET /api/v1/auth/github/start?return_to=/view/vw_...?claim=1
  v
Hooktry Worker
  |
  | state + PKCE challenge
  v
github.com/login/oauth/authorize
  |
  | code + state
  v
/api/v1/auth/github/callback
  |
  | code + client secret + PKCE verifier
  v
GitHub token exchange
  |
  | ephemeral user access token
  v
GET api.github.com/user
  |
  v
Hooktry user + personal workspace + session
~~~

The GitHub access token is used only to revalidate identity during that callback. It is never persisted by Hooktry.

## CSRF and PKCE

AUTH1 uses two independent protections.

### OAuth state

A cryptographically random `state` value is:

- stored in D1 only as a SHA-256 digest
- bound to the browser through an HttpOnly, Secure, SameSite=Lax cookie
- scoped to the GitHub auth path
- valid for ten minutes
- consumed exactly once

The stored state also carries a same-origin relative `return_to` path. Absolute or protocol-relative redirects are rejected.

### PKCE

A 32-byte random verifier remains only in a short-lived HttpOnly cookie.

Only its SHA-256 S256 challenge is sent in the authorization request. The raw verifier is returned server-side during the code exchange and is never stored in D1.

## Hooktry session

After GitHub identity validation, Hooktry creates its own opaque session.

~~~text
raw sess_... token
    |
    | browser cookie only
    v
SHA-256 digest
    |
    v
D1 auth_sessions
~~~

Session properties:

- HttpOnly
- Secure on HTTPS
- SameSite=Lax
- 30-day absolute lifetime for AUTH1
- raw session token is never persisted
- expired sessions are removed lazily and by scheduled cleanup

The browser never receives `CLAIM_INTERNAL_TOKEN`.

## Identity and workspace model

D1 stores:

- `auth_users`
- `auth_workspaces`
- `auth_sessions`
- `auth_oauth_states`

A GitHub numeric user ID is the external stable identity key. GitHub login and avatar are mutable profile attributes and are refreshed on every sign-in.

The first sign-in idempotently creates one personal workspace owned by the Hooktry user.

AUTH1 does not yet implement:

- teams
- invitations
- multiple workspaces per user
- role management
- SSO
- email identity
- password authentication

Those can extend the workspace model without changing anonymous claim semantics.

## Claim authority

The public claim capability remains:

~~~text
POST /claim/cl_<capability>
~~~

There are now two valid authority sources.

### Human browser

An authenticated Hooktry session supplies the personal workspace identity. The browser supplies the claim capability through the claim URL already held in its owner session.

### Deployment acceptance

The existing internal bearer authority remains available to production acceptance so CI can prove claim semantics without creating a fake human OAuth identity.

These authorities do not combine. A browser session never receives the internal token.

## WEB1 behavior

The owner browser already stores the provision response in session storage keyed by the view capability.

When the user chooses Claim:

1. WEB1 checks for an Hooktry session.
2. If authenticated, it posts the claim immediately.
3. Otherwise it starts GitHub OAuth with a same-origin return target:
   `/view/vw_<capability>?claim=1`.
4. The GitHub callback returns to that exact view.
5. Session storage still contains the owner-only claim capability.
6. WEB1 completes the claim.
7. The Hook becomes persistent without changing its ingress URL or history.

A browser that possesses only the `vw_` URL still cannot derive the hook or claim capability.

## After claim

AUTH1 proves the transition into a personal workspace. A claimed Hook must then become a normal durable workspace resource discoverable without the original anonymous browser session.

Workspace list/reopen/history/revoke/delete and independent ingress/view capability rotation are specified in [HOOK2 - Persistent Workspace Hook Inventory](persistent-workspace-hooks.md).

## Runtime configuration

GitHub identity uses:

~~~text
GITHUB_CLIENT_ID       # GitHub production environment variable
GITHUB_CLIENT_SECRET   # GitHub production environment secret / Worker secret
~~~

The ordinary Cloudflare deployment carries `GITHUB_CLIENT_ID` as a Worker variable.

The client secret is installed separately by the manual **Configure Cloudflare Auth** workflow. Ordinary deployments do not rewrite secrets.

`GET /healthz` exposes only:

~~~json
{
  "github_auth_configured": true
}
~~~

It never returns the client ID or client secret.

Production acceptance requires auth readiness before a release is considered converged.

## Domain migration

AUTH1 initially uses the current managed-cloud callback origin:

~~~text
https://hooktry-cloudflare.web33.workers.dev/api/v1/auth/github/callback
~~~

When DOMAIN1 moves the canonical product origin to `https://hooktry.com`, the GitHub OAuth App callback must move to:

~~~text
https://hooktry.com/api/v1/auth/github/callback
~~~

The identity/session model does not change.

## Non-goals

- storing GitHub access or refresh tokens
- requesting repository access
- using GitHub identity as an API credential
- browser access to internal claim authority
- solving team/workspace management
- making local single-binary usage require cloud authentication
