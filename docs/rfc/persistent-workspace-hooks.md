# HOOK2 - Persistent Workspace Hook Inventory

Status: accepted product/architecture direction  
Checked: 2026-10-02

## Goal

Complete the post-claim product model.

Anonymous-first gives Hooktry a fast create/use/claim flow. After claim, the Hook must become a normal durable workspace resource that can be rediscovered and managed from any authorized client.

Claim is therefore not the end of the lifecycle. It is the transition from capability-only discovery to workspace inventory.

## Lifecycle

~~~text
anonymous Hook
  -> create/use through hk_ / vw_ / cl_ capabilities
  -> claim into authenticated workspace
  -> same Hook remains addressable
  -> appears in Workspace Hook Inventory
  -> can be reopened, inspected, revoked, rotated, or deleted by workspace authority
~~~

The existing ingress URL and retained history remain unchanged across claim unless the user explicitly rotates or deletes them.

## Workspace inventory

Authenticated clients should be able to list and discover workspace-owned Hooks without possessing the anonymous browser wallet.

Inventory is the durable cross-device source for:

- browser
- CLI
- MCP/agent
- remote devbox
- CI or other workspace-authorized automation

At minimum, a workspace Hook summary should expose:

- Hook/Exposure identity
- stable display label/name when added
- created/claimed timestamps
- status
- request count / recent activity summary
- retention state/policy
- ingress status
- last Interaction time where available
- enough data to reopen the human viewer without leaking secret capabilities

## Reopen viewer

A claimed Hook should not require the original anonymous `vw_` capability to be manually preserved forever.

An authenticated workspace session may reopen the Hook viewer through workspace authorization and a stable resource route, for example conceptually:

~~~text
/workspace/hooks/<hook-id>
~~~

The exact public route is a UI concern. The important rule is that authenticated workspace authority can obtain/read the Hook's retained evidence without reconstructing or exposing the old anonymous view capability.

The original `vw_` bearer may remain valid according to policy until rotated/revoked, but Workspace Inventory becomes the normal discovery path after claim.

## History

A claimed Hook preserves the retained anonymous history that existed at claim time.

New Interactions continue into the same canonical Interaction/evidence model.

Do not fork anonymous and claimed history into separate stores or timelines.

~~~text
same Hook
  -> Interaction 1 (anonymous)
  -> Interaction 2 (anonymous)
  -> claim
  -> Interaction 3 (workspace-owned)
  -> ...
~~~

Viewer/replay/contracts/scenarios should continue to operate over one history subject to retention policy.

## Management operations

Workspace authority should eventually support:

### List

Discover workspace Hooks across devices and clients.

### Get / reopen

Open a Hook and inspect retained evidence/history.

### Revoke ingress

Stop future delivery through the current `hk_` capability without deleting historical evidence.

Revocation is a traffic-control operation, not necessarily data deletion.

### Rotate ingress capability

Issue a new `hk_` capability and invalidate the previous one while preserving the same Hook identity/history.

Useful when an ingress URL is accidentally exposed or a producer should be migrated.

### Rotate view capability

Invalidate an old shared `vw_` URL and issue a replacement without changing the Hook or ingress.

### Delete

Delete the Hook according to product retention/deletion rules, including retained payload/history and associated capability digests.

Delete must be explicit and distinguishable from ingress revoke.

## Capability rotation rules

Hook identity and bearer capabilities are different things.

~~~text
Hook ID / Exposure ID = durable resource identity
hk_ = ingress authority
vw_ = read/view authority
cl_ = one-time anonymous claim authority
~~~

After claim:

- `cl_` is consumed and no longer relevant
- `hk_` may remain stable until explicit rotation/revoke
- `vw_` may remain usable until policy/rotation says otherwise
- workspace authorization becomes an additional management/read authority

Rotating `hk_` must not rotate `vw_` implicitly.

Rotating `vw_` must not rotate `hk_` implicitly.

Capability separation remains intact after authentication.

## API direction

Do not create a separate claimed-Hook domain model.

Workspace inventory should expose the existing canonical Hook/Exposure resource through workspace-scoped operations.

Conceptual API shape:

~~~text
GET    /api/v1/workspaces/current/hooks
GET    /api/v1/workspaces/current/hooks/<id>
POST   /api/v1/workspaces/current/hooks/<id>/rotate-ingress
POST   /api/v1/workspaces/current/hooks/<id>/rotate-view
POST   /api/v1/workspaces/current/hooks/<id>/revoke
DELETE /api/v1/workspaces/current/hooks/<id>
~~~

Exact routes may differ. The contract matters more than the URL spelling.

Machine clients should receive structured stable resource identifiers and status, not browser-only URLs as the sole inventory representation.

## Browser UX

After authentication the left navigation can evolve from anonymous Recent Hooks into workspace-backed inventory:

~~~text
Hooks

Workspace
  payments-dev       14 requests  active
  github-events       3 requests  active
  old-test-hook      81 requests  revoked

Anonymous / Recent
  01a0...             expires in 4d

+ New Hook
~~~

Claim should move/link the anonymous entry into Workspace without losing the current viewer context.

## CLI and MCP UX

Authenticated CLI/MCP should support the same inventory semantics, for example conceptually:

~~~text
hooktry hooks
hooktry hook get <id>
hooktry hook revoke <id>
hooktry hook rotate <id> --ingress
hooktry hook delete <id>
~~~

The exact CLI syntax can evolve. Agents should not need browser state or the original anonymous capabilities to manage an already-claimed workspace Hook.

## Security

Workspace inventory must remain workspace-scoped.

Cross-workspace lookups should not disclose resource existence.

Management actions require explicit workspace scopes/authority.

Raw bearer capabilities should be revealed only when the user operation needs them.

Listing Hooks should not automatically dump `hk_` or `vw_` secrets into every response.

Prefer explicit reveal/rotate/copy operations where appropriate.

## Relationship to DISC1

DISC1 defines how an anonymous Hook is rediscovered before authentication.

HOOK2 defines the durable discovery/management model after claim.

~~~text
Local Capability Wallet / handoff
        -> claim
        -> Workspace Hook Inventory
~~~

## Relationship to AUTH1

AUTH1 proves the identity + claim transition into a personal workspace.

HOOK2 is the next product layer over that workspace: durable Hook inventory and lifecycle management.

## Suggested slices

~~~text
HOOK2.1 - workspace Hook list/get API
HOOK2.2 - browser Workspace Hooks list + reopen
HOOK2.3 - revoke/delete semantics
HOOK2.4 - ingress/view capability rotation
HOOK2.5 - CLI/MCP inventory and management
~~~

## Invariants

1. claim preserves Hook identity and existing history
2. claimed Hooks become discoverable through Workspace Inventory
3. authenticated discovery does not depend on browser local storage
4. revoke ingress does not silently delete history
5. delete and revoke are distinct operations
6. ingress and view capabilities rotate independently
7. workspace authority does not collapse capability separation
8. agents/CLI can manage claimed Hooks without browser state
9. anonymous and claimed Interactions remain one canonical evidence history
10. listing resources should not casually expose bearer secrets

## Non-goals

HOOK2 does not define team roles, billing, final retention tiers, custom domains, producer-specific signature configuration, or the full replay/contracts UI.
