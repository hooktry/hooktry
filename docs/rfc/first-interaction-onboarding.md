# WEB2 - First Interaction Onboarding

Status: accepted product/UX direction  
Checked: 2026-10-02

## Goal

Reduce the path from creating an ephemeral Hook to seeing the first real Interaction to one obvious action.

Target onboarding loop:

~~~text
New Hook
  -> Send test OR copy ready-to-run command
  -> Interaction appears live
  -> inspect body / headers / metadata
  -> copy ingress URL
  -> integrate real producer
~~~

The product should make this flow understandable without requiring the user to manually construct curl/HTTPie syntax.

## Ingress action surface

Near the Public ingress URL, expose explicit actions such as:

~~~text
Copy URL   cURL   HTTPie   Send test
~~~

`cURL` copies a ready-to-run command, for example:

~~~sh
curl 'https://.../hook/hk_...' \
  -H 'Content-Type: application/json' \
  -d '{"hello":"world"}'
~~~

`HTTPie` copies a ready-to-run command, for example:

~~~sh
http POST 'https://.../hook/hk_...' hello=world
~~~

The examples should be shell-safe, minimal, and use the actual Hook ingress capability.

Do not include view, claim, handoff, workspace, or internal control-plane capabilities in these examples.

## Send test must use the real ingress

`Send test` must not insert a fake Interaction directly into UI state or call an internal-only shortcut.

It should send a real HTTP request through the same public ingress path a real producer uses:

~~~text
WEB1
  -> public /hook/hk_... endpoint
  -> normal capture/persistence path
  -> realtime stream
  -> Interaction appears in WEB1
~~~

This makes onboarding itself an end-to-end product proof.

The test request should be clearly identifiable as Hooktry-generated evidence, for example through a small safe JSON body and a non-secret diagnostic header. It must still be captured exactly like any other request.

## First-Interaction metric

A useful product metric is time from Hook creation to first Interaction.

The design target is a first successful Interaction within roughly 10 seconds for a new user who chooses `Send test` or copies a generated command.

This is an onboarding target, not a network SLA.

## Request examples should exercise the model

Generated examples should make it easy to demonstrate:

- HTTP method
- nested path
- query string
- custom header
- JSON body

For example:

~~~sh
http POST 'HOOK_URL/orders?source=demo' \
  event=order.created \
  order_id:=42 \
  X-Demo:hooktry
~~~

This helps a user immediately see that Hooktry captures structured Interaction evidence rather than only a raw body.

## Evidence surfaced by the inspector

The inspector should continue to expose captured request evidence such as:

- method
- path
- query
- headers
- body
- body encoding/size
- durable Interaction identity and sequence
- receive time

Managed ingress may also contribute transport/platform evidence such as:

- User-Agent
- forwarded protocol
- host
- provider delivery/request identifiers
- Cloudflare/network metadata

That information is evidence about transport and provenance. It must not be treated as Hook ownership identity.

See DISC1 for the explicit `producer != owner` rule.

## From inspection to understanding

WEB2 should preserve a clean layering:

~~~text
raw Interaction evidence
  -> normalized transport/context evidence
  -> provider/event understanding
  -> replay / contracts / proof
~~~

The current Headers/Metadata UI is therefore not merely debugging chrome. It is the visible base of Hooktry's deeper evidence model.

Future understanding may identify event/provider semantics, important identifiers, signature status, correlations, or changes versus earlier requests, but it must remain derived from preserved evidence rather than replacing it.

## Capability safety

The ingress response remains send-only and should continue to return delivery metadata such as:

~~~json
{
  "ok": true,
  "interaction_id": "...",
  "sequence": 2
}
~~~

It must not return `view_url`, `claim_url`, handoff authority, or another capability that grants read/ownership access.

Generated terminal commands contain only the ingress capability.

## Relationship to WEB1

WEB1 proved the usable surface:

~~~text
create Hook
  -> live WebSocket viewer
  -> Interaction list
  -> body / headers / metadata inspector
~~~

WEB2 improves the first-use path without introducing a second browser model.

## Suggested implementation slices

~~~text
WEB2.1 - cURL / HTTPie copy actions
WEB2.2 - Send test through real ingress
WEB2.3 - first-Interaction onboarding state
WEB2.4 - transport/context presentation cleanup
~~~

## Non-goals

WEB2 does not make terminal commands ownership authority, infer identity from network metadata, bypass the normal ingress path for tests, or implement the full provider-understanding layer.
