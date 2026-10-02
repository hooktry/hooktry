# ACCESS7: Expose Product Workflow

Status: executable vertical slice  
Tracking: #33

ACCESS7 turns the Exposure primitive into an agent-friendly CLI workflow.

## Create

```sh
hooktry expose 3000
hooktry expose 3000 web
hooktry expose 3000 stripe --no-verify
```

The executable slice intentionally defaults to local `forward` + `private`. It therefore works without pretending that `relay.hooktry.test` is a deployed hosted service.

Output is one stable JSON object:

```json
{
  "exposure_id": "...",
  "name": "web",
  "url": "http://127.0.0.1:7777/exposed/...",
  "access": "private",
  "mode": "forward",
  "target_port": 3000,
  "verified": true
}
```

This is the same shape an agent can consume without scraping terminal prose.

## Verification

Unless `--no-verify` is supplied, the CLI requests `/_hooktry_verify` through the newly created exposure. Verification is fail-closed: only a successful response from the target through the HOOKTRY Boundary produces `verified: true`.

Creating the exposure and verifying the target are separate facts. A target without that route can therefore produce a valid exposure with `verified: false`.

## Manage

```sh
hooktry exposures
hooktry exposure-get <id>
hooktry exposure-revoke <id>
```

All successful commands emit JSON.

## Hosted relay

ACCESS3-6 already provide the relay transport, authenticated runtime registration, HTTP ingress, and resilience primitives. This CLI slice does not claim a hosted HOOKTRY relay exists. Wiring these primitives to a real deployment requires a control-plane endpoint that provisions relay exposure + capability and a TLS-protected relay address.

That deployment can extend this CLI without changing the Exposure, Boundary, Interaction, or structured-output model.
