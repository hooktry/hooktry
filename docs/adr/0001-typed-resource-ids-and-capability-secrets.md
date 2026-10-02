# ADR-0001: Typed resource IDs and capability secrets

Status: Accepted  
Date: 2026-10-02

## Context

Hooktry has two identifier classes with different security and operational properties:

1. resource identity, such as Exposure, Interaction, Run, Recording, Contract, or Execution identity;
2. bearer capabilities, such as the authority to send to a Hook, view captured traffic, or claim an anonymous Exposure.

These must not share one generator merely because both appear as strings.

Resource IDs benefit from:

- globally unique values;
- stable type information in logs, APIs, and agent output;
- chronological locality / k-sortability;
- compact punctuation-free rendering;
- lossless conversion to a standard UUID representation.

Bearer capabilities instead benefit from:

- maximum unpredictability;
- no embedded creation timestamp;
- no ordering semantics;
- compact punctuation-free rendering;
- explicit authority semantics;
- digest-only persistence where possible.

The previous anonymous capability formats moved from Base64URL to 32-character hex to improve copy/select ergonomics. Base64URL was URL-safe but could emit `-` and `_`, which are awkward in terminals, chats, and double-click selection. Hex solved punctuation but is longer than necessary and does not provide a shared visual language with typed resource IDs.

## Decision

### 1. Resource IDs use TypeID over UUIDv7

For durable resource identity, use the TypeID model:

```text
<type>_<26-char lowercase Crockford Base32 UUIDv7>
```

Examples:

```text
exp_01k...
int_01k...
run_01k...
```

The 128-bit payload is a standards-compliant UUIDv7. The external TypeID string is a lossless encoding of those UUID bytes, not a separately generated identifier.

Properties:

- UUIDv7 provides time-orderable identity under RFC 9562;
- the typed prefix prevents cross-resource ambiguity;
- lowercase Crockford Base32 avoids punctuation and ambiguous visual characters;
- 128 UUID bits encode to 26 characters instead of 32 hex characters or a 36-character dashed UUID;
- databases may store the native UUID bytes/type while API, logs, CLI, MCP, and UI expose the TypeID form.

Do not hand-roll UUIDv7 bit layout. Use maintained UUIDv7 / TypeID implementations and pin them normally through the project dependency lockfile.

For TypeScript, `typeid-js` is the reference implementation currently preferred for TypeID encoding/decoding. For Rust, continue using a maintained RFC 9562 UUIDv7 implementation and a maintained Crockford/TypeID codec; enforce compatibility with shared test vectors rather than relying on two independent handwritten encoders.

### 2. Capability secrets use random 128-bit Crockford Base32, not UUIDv7

Hooktry Hook/View/Claim capabilities are bearer secrets, not resource IDs.

Generate exactly 128 bits with a cryptographically secure random source and encode the 16 bytes into a fixed-width 26-character lowercase Crockford Base32 suffix.

Canonical forms:

```text
hook_<26-char-random-crockford-base32>
view_<26-char-random-crockford-base32>
claim_<26-char-random-crockford-base32>
```

Examples:

```text
hook_7j3...
view_a8m...
claim_2kp...
```

Do not use UUIDv7 for these values. A UUIDv7 intentionally embeds time/order information and therefore spends part of the 128-bit layout on non-secret structure. Capability authority does not benefit from ordering and should not reveal creation time.

The prefixes are semantic labels only. Authority comes from possession of the unpredictable token.

### 3. Identity and authority remain separate

A resource may therefore have both:

```text
Exposure identity:
exp_01k...

Hook authority:
hook_7j3...
```

The resource ID may safely appear in logs, database relationships, API objects, and evidence.

The capability token must be treated as a secret:

- do not use it as the database primary identity;
- persist only its digest when practical;
- redact it from telemetry and logs;
- never infer ownership from the typed prefix;
- rotate/revoke authority independently from resource identity.

### 4. Canonical Hooktry capability URLs use plural resource namespaces

New anonymous capability URLs are:

```text
https://hooktry.com/hooks/hook_<token>
https://hooktry.com/views/view_<token>
wss://hooktry.com/views/view_<token>
https://hooktry.com/claims/claim_<token>
```

The collection path and capability type intentionally repeat the concept at different layers:

- `/hooks` is the HTTP product/resource namespace;
- `hook_` identifies the opaque capability if copied outside its URL.

This is preferred over a root-level form such as:

```text
https://hooktry.com/hook_<token>
```

because root-level capability URLs consume the global product namespace and make future routing, documentation, caching, security policy, and product pages harder to evolve.

It is also preferred over singular-short forms such as:

```text
/hook/hk_...
/view/vw_...
/claim/cl_...
```

because the expanded vocabulary is clearer in logs, chat, support, and agent output.

### 5. Backward compatibility is parse-wide, generate-narrow

After implementation:

- generate only the new canonical format;
- continue resolving already-issued legacy `/hook/hk_...`, `/view/vw_...`, and `/claim/cl_...` capabilities for their normal lifetime;
- do not rewrite an already-issued Hook URL merely because the format changed;
- remove legacy route/token acceptance only after no valid issued capability can remain.

For the current five-day anonymous TTL, the compatibility window can normally be bounded to the maximum outstanding anonymous lifetime plus a safety margin. Claimed/persistent resources require explicit migration/rotation policy before legacy support is removed.

## Library and compatibility rule

The string format is a contract, not an implementation accident.

Maintain cross-runtime golden vectors that prove:

- UUIDv7 bytes -> TypeID string -> same UUIDv7 bytes;
- random 16 bytes -> lowercase Crockford Base32 capability suffix -> same 16 bytes;
- Rust and TypeScript produce the same canonical encoding;
- prefixes are validated independently from payload decoding;
- uppercase/non-canonical encodings may be accepted for parsing only if deliberately specified, but emitted values are always lowercase canonical form.

Prefer established libraries over local bit manipulation. A dependency is replaceable; the wire/storage contract is not.

## Alternatives considered

### Dashed UUIDv7

Rejected as the public representation because it is longer, not typed, and contains punctuation that hurts copy/select ergonomics.

### UUIDv7 with dashes stripped

Better ergonomically, but still lacks a typed standard representation and remains 32 hex characters instead of TypeID's 26-character Base32 suffix.

### Hex capability tokens

Secure and simple, but 32 characters are longer than the equivalent 128-bit Crockford Base32 representation.

### Base64URL capability tokens

Compact and secure, but may contain `-` and `_`, creating poor human-selection ergonomics.

### TypeID/UUIDv7 for capability secrets

Rejected because time-sortability and embedded creation time are undesirable for bearer authority.

### Root-level capability URLs

Rejected as the default because they pollute the top-level product namespace and couple a secret representation directly to root routing.

## Consequences

Positive:

- resource identity and authority have explicit, different semantics;
- IDs are typed, compact, sortable where useful, and easy to select/copy;
- capability tokens remain fully random and reveal no timestamp;
- the same visual grammar works across logs, APIs, CLI, MCP, and UI;
- the decision is portable across Rust, TypeScript, databases, and future runtimes.

Costs:

- existing Hooktry capability URLs need a compatibility migration;
- cross-language encoding vectors become part of the contract;
- resource-ID adoption is incremental because existing persisted UUIDv7 values must not be rewritten casually;
- additional dependencies are justified only when they implement the standard/codec boundary cleanly.

## Related

- [Anonymous-first Exposure and claim lifecycle](../rfc/anonymous-first-claim-later.md)
- RFC 9562 UUIDv7
- TypeID specification / reference implementations
