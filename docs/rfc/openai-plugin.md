# PLUGIN1 - OpenAI Plugin and Remote MCP Surface

Status: PLUGIN1 production accepted; authenticated follow-up in progress  
Date: 2026-10-02

## Goal

Make Hooktry installable as a ChatGPT and Codex plugin without weakening Hooktry's existing authority boundaries.

The plugin is an additional first-class product interface:

```text
Human        Software         Agent
  |             |               |
hooktry.com  api.hooktry.com  mcp.hooktry.com
                                  |
                                  +-- /mcp
```

The canonical production MCP URL is:

```text
https://mcp.hooktry.com/mcp
```

Do not publish a temporary `workers.dev`, `onrender.com`, or `api.hooktry.com/mcp` URL as the directory endpoint. OpenAI's current update flow does not allow changing a published MCP server URL without support intervention.

## Current OpenAI contract

This RFC follows the OpenAI plugin documentation current on 2026-10-02.

### Portable package

New packages use the Agent Plugins portable layout:

```text
plugin/
├── plugin.json
├── mcp.json
└── skills/
    └── get-started/
        └── SKILL.md
```

Root `plugin.json` and `mcp.json` are canonical for new packages. A `.codex-plugin/plugin.json` file is a compatibility fallback, not the primary format.

Do not add `.app.json` mappings or lifecycle hooks to the public submission package yet. OpenAI currently rejects public ZIP submissions containing app references or lifecycle hooks. UI can still be served by the remote MCP server later without putting an app mapping in the ZIP.

### Remote MCP transport

The public MCP server must:

- be available over production HTTPS;
- support MCP Streamable HTTP;
- use a stable URL, normally ending in `/mcp`;
- expose explicit user-intent tools rather than a generic executor;
- declare an output schema for structured tool results where applicable;
- set explicit `readOnlyHint`, `destructiveHint`, and `openWorldHint` annotations for every public tool;
- declare per-tool auth with `securitySchemes`.

The existing `hooktry mcp` stdio server remains supported for local agents and CLI workflows. It is not itself the public plugin server.

### Authentication

Private workspace data and user-authorized writes require request-scoped OAuth 2.1 conforming to the MCP authorization specification.

The public MCP service must never implement user identity by reading process-wide `HOOKTRY_TOKEN` or `HOOKTRY_APPROVER_TOKEN`. Those environment credentials remain valid for local/controlled operator processes only.

The OAuth resource identifier will be:

```text
https://mcp.hooktry.com
```

The resource server will eventually publish:

```text
https://mcp.hooktry.com/.well-known/oauth-protected-resource
```

Each authenticated tool will declare its scopes explicitly. Hooktry Workspace and scoped credential semantics remain authoritative underneath OAuth.

Prefer an established identity provider rather than implementing an authorization server from scratch.

### MCP Events

MCP Events are a separate follow-up slice, not part of the first transport slice.

ChatGPT MCP Events currently require protocol version `2026-07-28`. Hooktry's existing local MCP server advertises `2025-06-18`.

The event slice therefore includes an explicit protocol upgrade and adds:

```text
server/discover
events/list
events/subscribe
events/unsubscribe
```

Initial event candidates:

- `interaction.created`
- `scenario.completed`
- `scenario.failed`
- `approval.requested`
- `execution.completed`
- `exposure.expiring`

Event subscriptions must be durable, authenticated, idempotent, and delivered using signed HTTPS webhooks. Callback destinations must reject private/local networks and redirects.

## Product surface

The public plugin surface is intentionally smaller than the internal/local MCP catalog.

Do not mechanically publish every `/_hooktry/*` operation.

Public tools should correspond to recognizable user goals, for example:

```text
create_webhook_endpoint
list_interactions
get_interaction
replay_interaction
create_scenario
run_scenario
get_scenario_outcome
revoke_exposure
```

Exact names may evolve before public submission, but one tool must represent one reviewable operation.

The existing local MCP catalog remains broader because it serves trusted local automation and operator workflows.

## Authority boundary

The remote plugin must preserve these invariants:

1. No server-wide Hooktry user token is used to impersonate a plugin caller.
2. Authenticated tools derive Workspace and scopes from the current request.
3. `approval_decide` is not exposed in the first public authenticated surface.
4. Approval authority stays separable from execution authority.
5. Raw secrets and SecretRef values are never returned to the model.
6. Anonymous viewer and claim capabilities are not returned by the first public tool slice.
7. Public tool results contain only the minimum data needed for the user goal.

## Phased implementation

### PLUGIN1.A - Portable package

Add a dedicated package under `plugin/`:

- `plugin/plugin.json`
- `plugin/mcp.json`
- `plugin/skills/get-started/SKILL.md`

The package points to `https://mcp.hooktry.com/mcp`.

The package can be tested privately before it contains all public-directory metadata. Final public submission additionally requires the production website, support, privacy, and terms URLs plus review material.

### PLUGIN1.B - Remote Streamable HTTP transport

Add `/mcp` to the hosted service.

The first remote surface is stateless and no-auth. It exists to prove:

- Streamable HTTP reachability;
- MCP initialization;
- tool discovery;
- tool invocation through ChatGPT developer mode;
- production routing through `mcp.hooktry.com`.

It does not expose private Workspace state.

The first useful no-auth tool is:

```text
create_webhook_endpoint
```

It creates an ephemeral anonymous Hooktry ingress URL and returns only:

- Exposure ID;
- webhook URL;
- expiration;
- request/body/retention limits.

It intentionally does not return the anonymous principal, viewer capability, WebSocket viewer capability, or claim capability. Those are bearer capabilities and must not become ordinary model-visible output.

Annotations:

```text
readOnlyHint: false
destructiveHint: false
openWorldHint: false
idempotentHint: false
securitySchemes: [{ type: "noauth" }]
```

Creating a Hook mutates Hooktry state but is additive rather than destructive. It does not access arbitrary external entities.

### PLUGIN2 - OAuth 2.1 and profile

Add request-scoped OAuth and a read-only profile tool for connected-account identity.

Map OAuth scopes to the existing Workspace scope model rather than creating a second authorization model.

Candidate scopes:

```text
exposures:create
exposures:read
exposures:revoke
interactions:read
scenarios:read
scenarios:write
replays:create
requests:execute
```

### PLUGIN3 - Authenticated developer workflow

Add the curated private tools required for the primary cohort:

- product development;
- integration testing;
- pilots.

Do not expose raw low-level administration endpoints.

### EVENT1 - MCP Events

Upgrade protocol support to `2026-07-28`, persist subscriptions, verify callback endpoints, sign deliveries, and emit the first Hooktry-native events.

The first target workflow is:

```text
Create a webhook endpoint
    -> external system calls Hooktry
    -> interaction.created
    -> ChatGPT receives the event
    -> user-requested reasoning/action continues
```

### UI1 - Optional MCP Apps / Extensions

Add an interactive interaction/execution viewer only after the tool-only workflow is proven.

UI is optional. The remote MCP contract stays useful without it.

## Hosting and DNS

The MCP protocol is a separate first-class service namespace even if it initially shares the same physical deployment as the hosted relay.

```text
mcp.hooktry.com
    -> current Hooktry hosted deployment
    -> /mcp
```

Routing by hostname can be introduced later without changing the public MCP URL.

The service root may eventually provide a human-readable connection page while `/mcp` remains the machine endpoint.

## Domain verification

Before directory submission, OpenAI will provide a challenge token that must be served as exact plain text at:

```text
https://mcp.hooktry.com/.well-known/openai-apps-challenge
```

An eligible parent domain can also be used when allowed by the submission portal.

Do not hard-code a challenge value in source before OpenAI issues it.

## Submission readiness

Initial public MCP review currently requires:

- verified developer or business identity;
- production HTTPS MCP URL;
- successful current tool scan;
- domain verification;
- website URL;
- support URL;
- privacy policy URL;
- terms URL;
- exactly five positive test cases;
- exactly three negative test cases;
- demo recording URL;
- release notes;
- accurate tool annotations.

Credentials for reviewers stay in the secure dashboard and must not be committed to the plugin ZIP.

## Non-goals for PLUGIN1

PLUGIN1 does not add:

- OAuth/OIDC user login;
- MCP Events;
- public approval decisions;
- custom MCP Apps UI;
- Extensions;
- a new Hooktry authorization model;
- server-wide user credentials for remote calls;
- a second implementation of Hooktry domain semantics.

## Verification

PLUGIN1.B is accepted when:

1. Hosted tests prove `POST /mcp` initializes successfully.
2. `tools/list` exposes only the intended remote tool catalog.
3. Every public tool has title, description, explicit schemas, auth policy, and safety annotations.
4. `create_webhook_endpoint` provisions a real anonymous Hook.
5. Its MCP result does not contain viewer, claim, principal, or secret material.
6. The returned webhook URL accepts an HTTP request through the normal anonymous ingress path.
7. The portable package declares `https://mcp.hooktry.com/mcp`.
8. Existing stdio MCP tests remain unchanged and green.

## Production dogfood acceptance - 2026-10-02

PLUGIN1 has been proven through the real ChatGPT product, not only repository tests.

Observed mobile flow on iOS:

```text
Hooktry plugin page
    -> attached Hooktry App
    -> Try in chat
    -> ChatGPT Work
    -> explicit Connect Hooktry consent
    -> @Hooktry create endpoint request
    -> create_webhook_endpoint
    -> live Hooktry webhook URL returned
```

The plugin package still presents a `Desktop only` badge for the standalone imported MCP surface. That badge does not prevent the attached Hooktry App from being invoked on iOS through ChatGPT Work.

The no-auth MCP tool still requires the user's normal ChatGPT connection consent. This is product-level app consent, not Hooktry OAuth and not a Hooktry account login.

The successful mobile invocation returned the expected anonymous policy:

- five-day expiration;
- 100-request limit;
- 5 MiB maximum request body;
- 50 MiB retained body-data limit.

Cloudflare Workers Observability independently recorded the successful invocation at approximately `2026-10-02T10:47:20Z`:

```text
host: mcp.hooktry.com
path: /mcp
method: POST
mcp protocol: 2026-07-28
mcp method: tools/call
mcp tool: create_webhook_endpoint
client: openai-mcp/1.0.0
response: 200
worker outcome: ok
```

The ChatGPT UI reported one transient service error before retrying the same operation. No failed Hooktry Worker invocation was observed in the corresponding Cloudflare window; the only matching Worker request was the successful `200` call above. Current evidence therefore places that transient failure before the Hooktry Worker boundary, in the ChatGPT/app orchestration path rather than the Hooktry MCP handler.

This proof upgrades PLUGIN1.B from transport acceptance to first-party mobile product dogfood acceptance.

## References

- OpenAI - Package your plugin: https://developers.openai.com/plugins/build/plugins
- OpenAI - Build an MCP server: https://developers.openai.com/plugins/build/mcp-server
- OpenAI - Authentication: https://developers.openai.com/plugins/build/auth
- OpenAI - MCP Events: https://developers.openai.com/plugins/build/mcp-events
- OpenAI - Submit and publish: https://developers.openai.com/plugins/deploy/submission
- OpenAI - Plugin submission errors: https://developers.openai.com/plugins/deploy/submission-errors
