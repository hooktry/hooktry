# EXPOSURE2 - Stable Exposure Identity with Capability Rotation

Status: executable vertical slice  
Tracking: #101

An Exposure is a durable resource identity. A runtime capability is a short-lived credential for attaching a runtime to that resource. Their lifetimes must not be coupled.

Before EXPOSURE2, a restart after the runtime capability expired revoked the durable dogfood Exposure and provisioned a replacement. That changed both the Exposure ID and its public URL.

EXPOSURE2 separates those lifecycles:

```text
Exposure 01...
public URL /e/01...
        |
        | capability expires
        v
rotate capability
        |
        +-- revoke previous runtime capabilities
        +-- issue fresh capability for same Exposure ID
        +-- rotate encrypted SecretRef
        +-- update persisted capability expiry
        |
        v
same Exposure 01...
same public URL /e/01...
```

## Capability rotation

`CapabilityStore::rotate_exposure` performs revocation of existing capabilities and insertion of the replacement under the same backend lock/transaction.

The old capability can no longer authorize. The new capability is scoped to the same Exposure.

The async form keeps synchronous SQLite/Postgres access off Tokio workers.

## Dogfood reconciliation

For an active same-name dogfood Exposure:

- different target port means the desired resource changed, so the old Exposure is revoked and a new one may be provisioned
- matching target with a valid stored capability reuses the Exposure unchanged
- matching target with an expired, revoked, invalid, or missing stored capability rotates only the capability and reuses the Exposure identity

The rotated token is stored through the encrypted workspace SecretStore. It is never logged or returned in dogfood evidence.

## Boundary

This slice proves the lifecycle primitive and production dogfood reconciliation. It does not yet expose a general user-facing capability-renewal endpoint. That belongs behind explicit workspace authority/policy rather than being added implicitly to the public Exposure API.
