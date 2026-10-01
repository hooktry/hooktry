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

## Follow-up status

At the time SECRET2 was introduced, master-key rotation was deliberately out of scope. That limitation has since been addressed by later slices:

- KEYROT1 added versioned master-key loading and versioned approval fingerprints
- KEYRET1 added key usage status, CAS-protected secret rewrap, and retirement readiness checks
- KEYROT2 records the required first real production v1 -> v2 rotation before public release

See [KEYROT2 - First Production Master-Key Rotation](production-master-key-rotation.md).

SECRET2 still does not define historical logical secret generations, a hosted secret-management API, or a general KMS product. Those remain separate decisions.
