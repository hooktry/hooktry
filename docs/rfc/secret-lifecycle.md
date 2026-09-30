# SECRET2 - Explicit Secret Lifecycle

Status: executable vertical slice  
Tracking: #98

SECRET2 makes the lifecycle of an encrypted Workspace secret explicit without widening the authority of SECRET1 or changing its envelope format.

```text
put / capture
    |
    v
SecretRef
    |
    +-- metadata/list
    +-- rotate -> new SecretRef id, same Workspace/name
    +-- resolve -> plaintext only inside an authorized caller
    +-- delete -> resolution becomes NotFound
```

## Compatibility

`SecretStore::put` remains supported and keeps its existing semantics. It delegates to `rotate`, because SECRET1 already implemented replacement by `(workspace_id, name)` with a fresh UUIDv7 and fresh AES-256-GCM ciphertext.

This means existing BOOTSTRAP2 and EXEC2 callers do not need to change.

## Lifecycle operations

SECRET2 exposes:

- `rotate` / `rotate_async`
- `delete` / `delete_async`
- `metadata`
- `list_metadata` / `list_metadata_async`

Metadata contains only:

- SecretRef identity
- Workspace identity
- logical secret name
- envelope key version

It does not contain plaintext or ciphertext.

Deletion is scoped by both Workspace and name. Deleting a same-named secret in one Workspace cannot affect another Workspace.

## Async boundary

SQLite and synchronous Postgres operations remain behind the same Tokio `spawn_blocking` boundary introduced by ASYNC1. Hosted callers should use the async lifecycle methods.

## Not in this pass

SECRET2 deliberately does not add persisted timestamps, historical secret generations, master-key rotation, or a hosted secret-management API.

Those require separate decisions:

1. whether a logical SecretRef should have stable identity plus generations, or whether each rotation remains a new reference
2. how multiple master-key versions are loaded during re-encryption
3. what audit events are durable and which actors are authorized to view them

The current envelope remains key version 1 and existing encrypted rows remain compatible.
