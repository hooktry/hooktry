# First Public Release Gates

DWC: PROD/RELEASE1.1 RELEASE1 - first public release acceptance

This is the currently known production gate list for Hooktry's first public release. It is intentionally short and evidence-driven. A checked item means the production behavior has been proven, not merely implemented.

## Release candidate

- [ ] The intended release candidate is merged to `main`.
- [ ] Main CI is green.
- [ ] Render automatic deploy points at the same accepted `main` revision.
- [ ] Production health endpoints are green.
- [ ] The public Exposure URL/API contract is considered release-ready and no unreleased compatibility aliases are being carried forward accidentally.

## Production master-key gate

KEYROT2 is blocking for the first public release. See [KEYROT2 - First Production Master-Key Rotation](../rfc/production-master-key-rotation.md).

- [ ] A recoverable production database backup/snapshot is confirmed.
- [ ] A fresh 32-byte production root key v2 is generated through a secure operator path.
- [ ] v2 is introduced as active while v1 remains configured as previous.
- [ ] Production startup reports `master_key_version=2`.
- [ ] `hooktry key status` is captured without key material.
- [ ] `hooktry key rewrap 1` dry-run matches expectations.
- [ ] `hooktry key rewrap 1 --apply` completes without unresolved candidates.
- [ ] `hooktry key retire-check 1` reports `safe_to_retire=true`.
- [ ] v1 is removed from runtime `HOOKTRY_SECRETS_PREVIOUS_KEYS`.
- [ ] Production is redeployed successfully with v2 active.
- [ ] Final key status shows no dependency on v1.
- [ ] v1 recovery material is retained only for the explicit rollback/recovery window and is not present in Git, logs, issues, PRs, or release evidence.

## Final production proof

- [ ] Data-plane dogfood is green on the final production revision.
- [ ] Approval webhook delivery is green.
- [ ] Approval webhook redaction proof is green.
- [ ] Control-plane dogfood is green.
- [ ] Render reports the final deployment live.
- [ ] The accepted revision and production revision are recorded.
- [ ] No secret or master-key material appears in release artifacts or acceptance evidence.

## Release

Only after the blocking gates above are complete:

- [ ] create the first public release/tag
- [ ] publish release notes with the accepted revision
- [ ] record the end of the rollback/recovery window
- [ ] destroy retired v1 recovery material according to the operator policy when that window is closed

This checklist may grow as other explicit first-release gates are discovered. Adding a gate is preferable to relying on an undocumented operator assumption.
