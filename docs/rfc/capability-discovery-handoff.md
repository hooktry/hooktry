# DISC1 - Anonymous Capability Discovery and Cross-device Handoff

Status: accepted product/architecture rule  
Checked: 2026-10-02

## Context

Hooktry deliberately separates anonymous Hook authority into independent bearer capabilities:

~~~text
hk_...  -> ingress / send
vw_...  -> view / read
cl_...  -> claim
~~~

This creates the correct security boundary, but it also creates a discovery problem:

- a browser may create a Hook and later lose the tab
- a CLI may create a Hook on a laptop
- an agent may create a Hook inside a remote devbox
- CI may create a Hook without any browser session
- a request producer may be Stripe, GitHub, HTTPie, curl, another service, or an arbitrary remote machine

The browser must not infer ownership from the sender of a webhook request.

The product therefore needs an explicit distinction between:

1. who owns or manages a Hook
2. who sent a particular Interaction

Those are separate identities and often have no relationship.

## Decision

Hooktry uses three discovery layers:

~~~text
Anonymous, same browser
    -> Local Capability Wallet

Anonymous, another device / CLI / remote devbox
    -> One-time Cross-device Handoff
    -> Local Capability Wallet

Authenticated
    -> Workspace Inventory
~~~

These layers complement each other. They are not alternative ownership models.

## Producer identity is not Hook ownership

A captured Interaction may contain evidence such as:

- source IP
- User-Agent
- HTTP method
- headers
- request body
- Cloudflare/network metadata
- provider signatures where present

This evidence describes the request transport and may help identify or verify a producer.

It must not establish Hook ownership.

Examples:

~~~text
browser and HTTPie share one public IP
    != same owner

two clients share one NAT
    != same owner

request came from user's laptop
    != browser owns the Hook

request came from a logged-in user's devbox
    != infer ownership from network metadata
~~~

IP addresses, User-Agent strings, forwarded headers, browser fingerprints, and network proximity are never ownership authority.

## Ingress response rule

The ingress capability grants send authority only.

Therefore:

~~~text
POST /hook/hk_...
~~~

may return delivery/correlation metadata such as:

~~~json
{
  "ok": true,
  "interaction_id": "...",
  "sequence": 2
}
~~~

but must never return:

- the view capability
- the claim capability
- an owner handoff token
- a workspace credential
- another capability that can be exchanged for read or claim authority

Otherwise any third-party webhook producer that knows hk_ would gain read or ownership authority.

Required invariant:

~~~text
hk_ -> send only
vw_ -> read only
cl_ -> claim only
~~~

Capabilities may resolve to the same Exposure internally, but must not be derivable from each other.

## Layer 1 - Local Capability Wallet

When a browser creates an anonymous Hook, it receives the owner provision:

~~~text
hk_
vw_
cl_
expires_at
Exposure metadata
~~~

The browser may retain this provision in a local capability wallet so the user can rediscover anonymous Hooks after losing a tab.

The wallet should support:

- Recent Hooks
- expiry display
- request-count summary
- reopen viewer
- copy ingress URL
- copy viewer URL
- remove local entry
- automatic removal after expiry where practical

The first implementation may use browser local storage or IndexedDB.

Session storage alone is insufficient because it is lost when the tab/browser session disappears.

### Security properties

The wallet is local convenience state.

It is not server-side ownership authority.

The server still validates the bearer capabilities themselves.

A browser that loses its wallet cannot reconstruct vw_ or cl_ from hk_.

A browser that opens only:

~~~text
/view/vw_...
~~~

has read authority only unless it also possesses the owner provision locally.

## Layer 2 - Cross-device Handoff

A Hook created by CLI, MCP, an agent, CI, or a remote devbox cannot magically appear in a browser wallet.

Hooktry therefore needs an explicit one-time handoff primitive.

Example CLI flow:

~~~text
hooktry hook create

Created ephemeral Hook - expires in 5d

Ingress
  https://hooktry.com/hook/hk_...

Viewer
  https://hooktry.com/view/vw_...

Open in browser
  https://hooktry.com/open/ho_...

Send test
  curl ...
~~~

The open/handoff URL contains a distinct handoff capability.

Suggested prefix:

~~~text
ho_...
~~~

The handoff capability is not hk_, vw_, or cl_.

### Handoff semantics

A handoff token should be:

- high entropy
- short-lived
- one-time
- stored server-side only as a digest
- invalidated atomically after exchange
- scoped to exactly one anonymous Hook owner provision
- unusable as ingress
- unusable as ordinary view capability after exchange

Example:

~~~text
remote devbox
    |
    | POST /api/v1/hooks
    v
Hooktry
    |
    +-- hk_
    +-- vw_
    +-- cl_
    +-- ho_...  one-time handoff
              |
              v
       laptop browser
              |
       GET /open/ho_...
              |
        atomic exchange
              |
              v
     Local Capability Wallet
~~~

After successful exchange the browser receives the owner-side provision needed to manage that anonymous Hook locally.

The ho_ token must not remain a reusable permanent owner URL.

### Pairing code variant

A short human-entered code may be added later:

~~~text
7KFM-P2RD
~~~

but it must resolve to a high-entropy server-side handoff capability and must have aggressive TTL/rate limits.

The short code itself must not weaken the underlying capability model.

## Layer 3 - Authenticated Workspace Inventory

After authentication, discovery should no longer depend on a specific browser wallet.

A Hook owned by a workspace belongs to server-side inventory:

~~~text
User
  |
Workspace
  |
Hook
~~~

Any authorized client may then create or discover Hooks:

~~~text
browser
CLI
MCP
remote devbox
agent
CI
~~~

For example:

~~~text
hooktry login
hooktry hook create
~~~

creates a workspace-owned Hook.

The browser can later query the authenticated inventory and find it even if the Hook was created elsewhere.

This is the durable cross-device model.

The local capability wallet remains useful for anonymous resources and fast handoff, but it does not replace Workspace Inventory.

## Anonymous to authenticated transition

The lifecycle remains:

~~~text
anonymous Hook
    |
    +-- browser wallet
    |        or
    +-- cross-device handoff
    |
    v
claim capability + authenticated workspace authority
    |
    v
same Hook becomes workspace-owned
~~~

Claim must preserve the existing public ingress capability and retained history unless explicit rotation is requested.

Once claimed, the Hook should appear in Workspace Inventory.

The anonymous wallet entry may then be replaced by or linked to the authenticated resource.

## Cookie and anonymous principal boundary

The existing anonymous principal cookie/header may be used for:

- abuse controls
- quota accounting
- convenience rediscovery hints

It must not authorize:

- viewing a Hook without vw_
- claiming a Hook without cl_
- recovering cl_
- reconstructing owner authority
- cross-device ownership

Cookie equality means only "same anonymous client context", not ownership proof.

## Browser UX

For browser-created anonymous Hooks, WEB should evolve from a single-tab owner session to:

~~~text
Hooks

Recent
  01a0f7f3...  2 requests  expires in 5d
  01a0f8b2...  8 requests  expires in 4d

+ New Hook
~~~

An owner view should expose separately:

~~~text
Public ingress   https://.../hook/hk_...   Copy
Viewer URL       https://.../view/vw_...   Copy
~~~

The viewer URL can be bookmarked or shared independently.

The claim capability should not be displayed casually as plain text unless there is a clear advanced-use reason.

## CLI / agent UX

CLI and agent-oriented creation should return structured capabilities plus human-friendly handoff.

Machine-readable example:

~~~json
{
  "exposure_id": "...",
  "hook_url": "https://hooktry.com/hook/hk_...",
  "view_url": "https://hooktry.com/view/vw_...",
  "claim_url": "https://hooktry.com/claim/cl_...",
  "handoff_url": "https://hooktry.com/open/ho_...",
  "expires_at_unix_seconds": 0
}
~~~

Human CLI output may additionally render ready-to-run curl/HTTPie examples.

Agents should never need browser state to create or use a Hook.

## Remote devbox example

A user works inside a remote development environment:

~~~text
remote devbox
    |
hooktry hook create
    |
    +-- ingress URL used by service under development
    |
    +-- handoff URL copied to laptop
                      |
                      v
                local browser
                      |
              owner wallet entry
                      |
              live Interaction view
~~~

No attempt is made to match the devbox IP to the laptop IP.

## Relationship to request evidence

Hooktry may later derive stronger producer identity from explicit verification mechanisms, for example:

- Stripe signature verification
- GitHub webhook signatures
- mTLS identity
- signed Hooktry agent/runtime metadata
- provider-specific authenticated integrations

Those are evidence about the producer.

They still do not automatically imply Hook ownership.

## Relationship to existing RFCs

DISC1 extends the anonymous lifecycle defined in [Anonymous-first Exposure and claim lifecycle](anonymous-first-claim-later.md).

It extends WEB1 beyond same-tab session persistence in [Shared Web Surface](shared-web-surface.md).

Authenticated Workspace Inventory builds on the existing Workspace and credential model rather than creating another ownership subsystem.

The public capability separation remains unchanged.

## Suggested implementation slices

Do not implement everything at once.

Suggested order:

~~~text
DISC1.1 - Browser Local Capability Wallet / Recent Hooks

DISC1.2 - Owner viewer URL + copy/open UX

DISC1.3 - One-time ho_ handoff capability

DISC1.4 - CLI/MCP create output includes handoff_url

DISC1.5 - Authenticated Workspace Hook Inventory
~~~

The one-time handoff should be introduced only after the wallet semantics are stable enough to receive it.

## Invariants

1. ingress authority never grants read authority
2. ingress responses never reveal vw_, cl_, or ho_
3. view authority never grants claim authority
4. request transport metadata never establishes Hook ownership
5. cookies/client identifiers never establish Hook ownership
6. browser wallet is convenience state, not server-side ownership authority
7. cross-device anonymous recovery requires explicit handoff
8. handoff is short-lived and one-time
9. authenticated discovery comes from Workspace Inventory
10. claim preserves the same Hook and history
11. capabilities remain non-derivable from one another
12. agents and CLI do not depend on browser state

## Non-goals

DISC1 does not:

- infer ownership from IP addresses
- fingerprint users across devices
- expose read capability to webhook producers
- turn anonymous cookies into authentication
- synchronize anonymous capability wallets implicitly across devices
- replace authenticated Workspace Inventory
- make the handoff URL a permanent owner URL
