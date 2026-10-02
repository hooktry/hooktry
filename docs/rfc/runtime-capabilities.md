# ACCESS5: Runtime Capabilities

Status: executable vertical slice  
Tracking: #23

ACCESS5 separates routing information from authority.

An exposure id and its public ingress URL identify where traffic should go. They do not authorize a runtime to attach to that exposure.

## Runtime capability

The relay control plane issues an opaque, short-lived capability scoped to exactly one exposure.

```text
control plane
    |
    | issue(exposure_id, ttl)
    v
hooktry_rt_<opaque>
    |
    | private runtime registration channel
    v
relay transport
    |
    | authorize before RelayBroker::register
    v
connected runtime
```

Properties:

- opaque token
- exactly one exposure scope
- expiry
- explicit revocation
- never embedded in the public webhook URL
- never forwarded to the local target
- independent from GitHub, Google, OIDC, or any future workspace identity provider

## Public ingress remains public by design

A public webhook exposure exists specifically so Stripe, GitHub, Twilio, or another external system can call it without knowing an Hooktry runtime credential.

The sensitive action is not calling a public exposure. The sensitive action is claiming the runtime side of that exposure and receiving its traffic.

## Threats covered by this slice

- guessing or learning an exposure id does not let an attacker register a runtime
- a capability for exposure A cannot register exposure B
- expired credentials fail closed
- revoked credentials fail closed
- invalid credentials fail before broker registration

## Deferred

Production capability persistence, hashing at rest, rotation/grace periods, workspace identity, audit logs, TLS transport security, and rate limiting remain deployment/ACCESS6 concerns.
