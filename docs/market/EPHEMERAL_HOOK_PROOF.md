# Ephemeral SaaS Hook V1 - Production Dogfood Proof

Checked: 2026-10-02

## What was proven

A manually created anonymous Hook in the production Cloudflare deployment was exercised from HTTPie and observed live in WEB1.

Observed production flow:

~~~text
HTTPie
  -> public hk_ ingress
  -> Cloudflare Worker
  -> Interaction persisted
  -> realtime WebSocket delivery
  -> WEB1 Interaction list
  -> Body / Headers / Metadata inspector
~~~

The ingress returned normal delivery metadata including an Interaction ID and sequence number. The same Interaction identity was visible in the browser inspector.

## First request

A body-less POST proved the empty-body case:

- HTTP 200
- `ok: true`
- sequence 1
- body size 0 B
- live Interaction appeared in WEB1
- captured headers were inspectable

## Follow-up request

A second HTTPie request used an additional path/query and JSON fields, proving that the same Hook accepts repeated traffic and increments durable sequence.

The important result is not the exact demo values. It is the end-to-end correspondence between the terminal request, persisted Interaction, and browser evidence.

## Transport evidence observation

The production inspector exposed real transport evidence, including HTTPie User-Agent and Cloudflare/forwarding metadata.

This confirms that Hooktry already has enough raw evidence to build a later understanding layer without relying on application instrumentation.

Transport metadata is evidence, not owner identity.

## Product conclusion

Ephemeral SaaS Hook V1 is proven end-to-end for the current slice:

~~~text
create
  -> receive
  -> persist
  -> push live
  -> inspect
~~~

This proof supports improving onboarding around the already-working path rather than inventing a parallel demo mechanism.

See `docs/rfc/first-interaction-onboarding.md` for WEB2.

## Evidence limits

This dogfood proof does not establish external customer demand, production reliability SLA, final retention policy, provider-signature understanding, or the future authenticated Free/Paid model.
