# KEYROT2 - First Production Master-Key Rotation

Status: proposed release gate  
Tracking: #158

KEYROT1 added versioned root keys. KEYRET1 added status, CAS-protected secret rewrap, and proof that a historical key is safe to retire.

Production intentionally still runs with master key version 1.

This RFC records the decision that HOOKTRY must complete one real production root-key rotation from v1 to v2 before the first public release.

## Why before the first public release

The mechanism is already implemented and tested, but a production key lifecycle is not fully proven until the real storage, deployment configuration, encrypted rows, approvals, webhook flow, and rollback path have all survived an actual rotation.

Before public release the blast radius is still small. After release, the same operation may affect external users, long-lived integrations, and a larger amount of encrypted state.

The first production rotation is therefore a release gate, not an optional post-launch hardening task.

## Why not rotate during ordinary feature work

A root-key rotation changes production authority over encrypted state.

It must not be mixed into an unrelated API, URL, UI, or feature deploy. The release candidate and public contract should first be stable enough that any production failure can be attributed to the key operation rather than an unrelated code change.

KEYROT2 is an explicit production operation with its own evidence and rollback boundary.

## Preconditions

Before starting KEYROT2:

- the intended release candidate is on `main`
- CI is green
- Render is serving the same `main` revision
- the public Exposure URL/API contract is considered release-ready
- KEYROT1 and KEYRET1 remain green
- a current production database backup or snapshot exists and is recoverable
- the operator has a secure way to create and store a fresh 32-byte root key
- the current v1 key remains available for rollback
- no key material will be placed in Git, issues, PRs, chat logs, command output, or acceptance evidence

## Rotation model

The safe transition is:

```text
v1 active
    |
    v
v2 active + v1 previous
    |
    +--> status
    +--> dry-run rewrap
    +--> apply rewrap
    +--> retire-check v1
    |
    v
safe_to_retire=true
    |
    v
v2 active only
```

Never move directly from `v1 active` to `v2 active only`.

## Production procedure

### 1. Establish the release candidate

Record:

- the `main` commit
- the corresponding Render deploy
- the backup/snapshot identifier
- the start of the maintenance operation

No secret values belong in this evidence.

### 2. Generate v2

Generate a fresh cryptographically random 32-byte root key through an operator-controlled secure path.

The key is stored only in the production secret/configuration system and the operator's approved recovery location.

### 3. Introduce v2 without retiring v1

Production configuration becomes conceptually:

```text
HOOKTRY_SECRETS_KEY=<v2>
HOOKTRY_SECRETS_KEY_VERSION=2
HOOKTRY_SECRETS_PREVIOUS_KEYS=1:<v1>
```

Deploy this configuration and verify that startup reports:

```text
master_key_version=2
```

At this point old ciphertext and old keyed approvals must still be usable through v1.

### 4. Inspect

Run:

```sh
hooktry key status
```

The status output is the baseline evidence for the migration.

### 5. Dry-run the rewrap

Run:

```sh
hooktry key rewrap 1
```

This must report candidates without changing ciphertext.

If the result is unexpected, stop here. v1 is still retained and no destructive retirement has occurred.

### 6. Rewrap secrets

Run:

```sh
hooktry key rewrap 1 --apply
```

Rewrap is CAS-protected. A concurrently changed secret must be skipped rather than overwritten.

Repeat status/dry-run as needed until no encrypted secret remains on v1.

### 7. Resolve approval dependencies

Run:

```sh
hooktry key retire-check 1
```

v1 cannot be retired while any of the following remain:

- encrypted secrets on v1
- pending approvals that depend on v1
- approved but not yet terminal approvals that depend on v1
- malformed actionable keyed approval fingerprints

Denied and consumed approvals are terminal and do not block retirement.

### 8. Retire v1 from runtime configuration

Only after:

```text
safe_to_retire=true
```

remove v1 from `HOOKTRY_SECRETS_PREVIOUS_KEYS`.

Do not destroy the securely retained v1 recovery material yet.

Deploy again.

### 9. Final proof

After v1 has been removed from runtime configuration:

- startup reports `master_key_version=2`
- `hooktry key status` is clean
- production health is green
- data-plane dogfood is green
- approval webhook dogfood is green
- redaction proof is green
- control-plane dogfood is green
- the deployed Render commit equals the accepted release-candidate revision or an explicitly documented rotation-only follow-up revision

The first public release may proceed only after this proof is recorded.

## Rollback

Before runtime retirement of v1, rollback is straightforward:

- keep v1 configured as a previous key
- restore v1 as active if necessary
- redeploy
- verify production health and dogfood

After v1 has been removed from runtime configuration, rollback may require re-adding the securely retained v1 key.

For that reason, destruction of v1 is a separate final action after the release rollback/recovery window is explicitly closed.

## Acceptance evidence

KEYROT2 is complete only when the tracking issue contains non-secret evidence for:

- release-candidate `main` revision
- green CI
- production backup/snapshot confirmation
- Render deployment with v2 active
- pre-rewrap key status
- rewrap dry-run result
- applied rewrap result
- `retire-check 1` with `safe_to_retire=true`
- v1 removed from runtime previous-key configuration
- final production status
- final data-plane/control-plane/webhook dogfood
- confirmation that no key material appeared in logs or development artifacts

## Non-goals

This RFC does not:

- rotate production keys now
- automate key generation
- automate destructive key retirement
- change the SecretStore envelope format
- change anonymous Exposure URL/API semantics
- define a general organization-wide key-management or KMS product

Those can be separate follow-up decisions.
