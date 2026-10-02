# RFC: Service Access and Exposures

Status: proposed  
Tracking: #5

## Summary

HOOKTRY should make a service reachable without treating reachability as a separate concern from interaction evidence.

The canonical primitive is **Exposure**: a routable access path that connects a client or external system to an HOOKTRY Boundary and, through that Boundary, to a target service.

An Exposure is not a generic tunnel. Traffic exposed through HOOKTRY must still participate in the HOOKTRY lifecycle:

**Observe -> Control -> Replay -> Assert**

This keeps service access inside the product boundary instead of turning HOOKTRY into a remote-development platform.

## Motivation

Modern development increasingly happens in isolated runtimes: local containers, CI jobs, remote devboxes, VMs, sandboxes, and agent environments.

Those runtimes often contain HTTP services that need to be reached by something outside the runtime:

- Stripe, GitHub, Slack, or another provider needs to deliver a webhook.
- A browser or teammate needs to open the application.
- A test runner needs a private path to a service.
- A coding agent needs to expose an application and return a usable URL.
- An integration test needs a stable ingress address while HOOKTRY captures evidence.

Today HOOKTRY can capture HTTP boundary traffic, persist it, record it, and replay it. It does not yet model how an external caller reaches that boundary.

## Product boundary

HOOKTRY should borrow the useful interaction model from remote-development products but not copy their scope.

Generic remote development asks:

> How do I reach port 3000 in this machine?

HOOKTRY asks:

> How do I make this integration boundary reachable, preserve its access policy, and turn traffic through it into deterministic evidence?

The distinction is important.

If traffic bypasses a Boundary, reachability is infrastructure and does not belong in HOOKTRY core.

If traffic enters through a Boundary and can therefore be observed, controlled, replayed, and asserted, it belongs in HOOKTRY.

## Canonical primitive

### Exposure

An Exposure describes one active route into a Boundary.

Proposed shape:

```json
{
  "id": "exp_...",
  "session_id": "...",
  "name": "web",
  "protocol": "http",
  "mode": "relay",
  "access": "private",
  "target": {
    "host": "127.0.0.1",
    "port": 3000
  },
  "boundary": {
    "protocol": "http"
  },
  "url": "https://...",
  "state": "active",
  "created_at": "...",
  "expires_at": null
}
```

The exact identifier format is not fixed by this RFC.

### Exposure mode

Initial modes:

- `forward` - private client-side forwarding. Lifetime is tied to the forwarding process/session. It may support arbitrary TCP later.
- `relay` - an HTTP(S) ingress URL backed by an HOOKTRY relay/provider.

The model must not assume that the provider is HOOKTRY Cloud. Providers may include a local runtime, a self-hosted relay, a cloud relay, or an integration with an existing environment.

### Access policy

Access is independent of transport and URL.

Initial policy vocabulary:

- `private` - only the authenticated owner/runtime may access it.
- `workspace` - authenticated members of the workspace may access it.
- `public` - reachable without HOOKTRY user authentication.

`public` must always be explicit.

Provider-specific webhook authentication is separate. For example, a public webhook endpoint may still be protected by Stripe or GitHub signatures.

### Lifecycle

An Exposure has an explicit lifecycle:

```text
creating -> active -> revoked
                  -> expired
                  -> failed
```

Revocation and expiry are first-class because URLs are capabilities.

## Request path

The important invariant is that an exposed request does not bypass HOOKTRY.

```text
external caller
      |
      v
Exposure
      |
      v
Boundary
      |
      +--> Interaction evidence
      |
      +--> controls / faults / policy
      |
      v
target service
      |
      v
Boundary response capture
```

This is what makes the feature materially different from a generic tunnel.

## Webhook correctness

The HTTP implementation must preserve request semantics closely enough that provider signatures remain verifiable by the target application.

In particular:

- preserve raw body bytes
- preserve relevant headers
- do not silently re-encode JSON
- retain method and path
- define forwarding-header behavior explicitly
- capture the response returned by the target

HOOKTRY may later verify signatures itself as an optional Boundary capability, but the relay must not make application-side verification impossible.

## Agent-native interface

The primary UX should support outcome-oriented agent requests.

Example:

```text
expose the port my web app runs on and give me its URL
```

The agent-facing result should be structured and deterministic:

```json
{
  "exposure_id": "exp_...",
  "url": "https://...",
  "access": "private",
  "target_port": 3000,
  "verified": true
}
```

Agents should not need to scrape human-oriented CLI tables.

Every mutating operation should support machine-readable output.

## CLI/API direction

Possible CLI vocabulary:

```bash
hooktry expose --port 3000 --name web
hooktry expose list
hooktry expose get web
hooktry expose revoke web
hooktry expose access web --mode workspace
```

The public API should map to the same domain model rather than duplicating semantics.

The final command names are intentionally not frozen by this RFC.

## Provider boundary

Core should not contain assumptions about DNS, TLS termination, NAT traversal, or a specific tunnel vendor.

A provider-neutral boundary should own those details.

Conceptually:

```rust
trait ExposureProvider {
    async fn create(&self, request: CreateExposure) -> Result<ProvisionedExposure>;
    async fn revoke(&self, exposure: &Exposure) -> Result<()>;
}
```

The domain owns desired semantics. Providers own reachability mechanics.

## Hosted HOOKTRY relay

A future hosted provider can give exposures an HOOKTRY-owned URL.

Examples only:

```text
https://<opaque-id>.in.hooktry.com
https://<name>-<session>.relay.hooktry.com
```

The hostname format is deliberately not decided here.

The relay should authenticate the runtime using a short-lived credential and multiplex incoming HTTP requests over an outbound connection from the runtime. This avoids requiring inbound network access to the developer machine, CI runner, devbox, or agent sandbox.

## Local/private forwarding

Private forwarding remains useful even when a hosted relay exists.

Use cases:

- inspect a remote runtime without publishing it
- connect a local browser or tool to an isolated service
- access non-HTTP protocols later
- keep traffic entirely inside a trusted network

The first HTTP-oriented slice does not require generic TCP support.

## Documentation discovery

HOOKTRY documentation should expose a machine-readable index at:

```text
https://hooktry.com/docs/llms.txt
```

The index should enumerate canonical documentation pages and give agents a stable discovery entrypoint before they navigate individual pages.

Agent-facing documentation should also include outcome prompts such as:

```text
Expose the HTTP service started by this project and give me the verified URL.
```

This is a documentation contract, not a replacement for a structured API or MCP surface.

## Vertical slice: ACCESS1

ACCESS1 should prove the abstraction before building a full public tunnel network.

1. Add canonical `Exposure` domain types.
2. Add a provider-neutral exposure service.
3. Add create/list/get/revoke API operations.
4. Use a deterministic fake provider for domain/integration tests.
5. Add one runtime/local implementation sufficient to prove lifecycle semantics.
6. Route exposed HTTP traffic through the existing HTTP Boundary.
7. Assert that the resulting request is stored as an `Interaction`.
8. Return structured JSON suitable for agents.
9. Add docs describing the agent workflow.
10. Keep hosted `hooktry.com` relay provisioning behind the provider boundary.

## Follow-ups

After ACCESS1:

- hosted relay with HOOKTRY-managed TLS
- workspace/private authentication
- explicit public webhook mode
- expiry and leases
- stable aliases
- browser access
- target health/readiness verification
- provider signature helpers
- MCP/agent skill
- automatic detection of listening HTTP services
- TCP forwarding
- UI for active exposures and captured interactions

## Inspiration

Namespace Devbox service access is a useful reference because it separates temporary private port forwarding from persistent HTTP URLs and exposes the workflow to coding agents.

HOOKTRY adopts the interaction pattern, not the product boundary. The HOOKTRY-specific value is that the route terminates in a programmable Boundary and therefore produces evidence that can be replayed and asserted.
