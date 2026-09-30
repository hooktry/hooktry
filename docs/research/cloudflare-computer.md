# Research: Cloudflare Computer as an ORTYO execution substrate

Status: Research note  
Date: 2026-09-30  
Decision: None  
Commitment: None

## Question

Can `@cloudflare/computer` provide a useful execution substrate for ORTYO without turning ORTYO into a Cloudflare-specific devbox or weakening the existing boundary/evidence model?

## Short answer

Yes, as an experimental provider behind an ORTYO-owned abstraction.

No, as a replacement for ORTYO's domain model or as a production dependency today.

`@cloudflare/computer` is a strong candidate for agent-scale workspaces because it combines a durable virtual filesystem with multiple execution backends. It also aligns with the provider-neutral direction already recorded in `EXEC1`. However, it is explicitly preview software, its API is unstable, its container path has important lifecycle and I/O limitations, and its `Workspace` abstraction is broader than ORTYO's current bounded HTTP execution contract.

The integration should therefore be treated as a provider experiment, not as a new ORTYO primitive.

## What Cloudflare Computer is today

The current package centers on a `Workspace` hosted by a Durable Object.

The Durable Object owns the authoritative virtual filesystem in SQLite. Execution backends operate against that filesystem through `workspace.runtime`.

Current backends include:

- a Cloudflare Container backend with a full Linux userland
- a Dynamic Worker shell backend based on `just-bash`
- a Dynamic Worker JavaScript backend for isolated ECMAScript modules with structured input/results

The container backend projects the Durable Object filesystem into the container through FUSE. A `computerd` process in the container synchronizes changes with the Durable Object over capnweb RPC.

The package also exposes filesystem operations, Git support, agent tools such as `read`, `write`, `edit`, `ls`, and `exec`, and artifact/file sharing helpers.

A workspace can register more than one backend under stable IDs, and execution is selected per call through one runtime entry point.

## Shipped behavior versus forward-looking design

The repository is unusually explicit that the project is preview-only:

- APIs are unstable.
- It is suitable for experiments, exploration, and prototypes.
- It is not recommended for production use yet.
- Parts of the design documentation describe intended future behavior rather than shipped behavior.

That distinction matters for ORTYO. We should integrate only against behavior we verify in a vertical slice, not against design documents alone.

## Fit with ORTYO

ORTYO's core remains:

```text
Observe -> Control -> Replay -> Assert
```

Cloudflare Computer belongs below that lifecycle as optional compute.

A useful layering is:

```text
ORTYO domain
  Boundary
  Session
  Interaction
  Recording
  Contract
  Scenario
       |
       v
ORTYO execution abstractions
  HttpExecutionProvider       <- EXEC1 today
  SandboxRuntimeProvider      <- possible future abstraction
       |
       +-- CloudflareComputerProvider
       +-- RenderProvider
       +-- BYOCProvider
       +-- future providers
```

The important point is that `@cloudflare/computer` should not become ORTYO's `Workspace` type and should not leak into canonical evidence contracts.

## Do not overload EXEC1

`EXEC1` currently models a deliberately narrow operation:

```text
bounded HTTP Request -> ExecutionProvider -> ExecutionEvidence
```

It includes strict HTTP-specific safety semantics such as destination validation, redirect denial, body limits, timeout limits, secret header resolution, and evidence redaction.

`@cloudflare/computer` exposes a much broader capability: arbitrary filesystem mutation and command/module execution.

Those are not the same authority.

If ORTYO adopts sandbox execution, it should introduce a separate provider boundary rather than silently expanding the meaning of the existing `ExecutionProvider`.

A future shape could be:

```text
SandboxRuntimeProvider
  create(runtime_spec)
  exec(command_or_module)
  read(path)
  write(path)
  stop()
  destroy()
```

Persistence/snapshot semantics should remain capability-driven because not every provider will implement them the same way.

## Where Computer is especially interesting for ORTYO

### 1. One durable workspace across cheap and heavy execution

File and Git operations can use a Dynamic Worker backend while commands requiring Linux, package managers, compilers, or native binaries can use a Container backend against the same logical filesystem.

That matches an agent workflow better than forcing every small operation through a VM/container.

### 2. Provider-controlled egress

The Computer repository includes an egress example that applies matching `none`, `all`, or custom policies across its container, Worker-shell, and Worker-JavaScript backends.

Cloudflare Containers also support outbound interception, allow/deny host gates, and default-deny behavior.

This is directly relevant to ORTYO because a sandbox must not become an unrestricted proxy. A useful future experiment is to route selected outbound traffic through an ORTYO Boundary so execution itself produces canonical Interaction evidence.

### 3. Git-native agent workspace

The package includes a typed Git interface and a shell `git` command backed by the same workspace state. That can support an agent session which checks out a repository, changes files, runs verification, and preserves the resulting workspace state.

### 4. Evidence-friendly supervision

Cloudflare's architecture separates durable state in the Durable Object from replaceable execution in the container. That is compatible with ORTYO's preference for durable evidence outside the thing being executed.

The container can fail or be replaced while ORTYO records execution/session evidence independently.

## Important distinction: Computer filesystem versus Container snapshots

The new Cloudflare Containers `durable_object` scheduling path separately introduces native filesystem snapshots.

These are not the same thing as the `@cloudflare/computer` virtual filesystem:

- `@cloudflare/computer`: authoritative workspace state is in Durable Object SQLite; the container sees a synchronized FUSE projection.
- Container snapshots: the container filesystem itself is checkpointed and can later seed one or many new containers.

Potentially they complement each other:

```text
container snapshot
  -> prepared OS/toolchain/dependency baseline

Computer Workspace
  -> durable task/repository/user file state
```

But the current Computer documentation does not establish native Container snapshots as part of its persistence model. We should not design ORTYO as if that integration already exists.

## Current limitations that matter to us

### Preview stability

The package states that it is not production-ready and that APIs can change.

### Workspace size

The documented workspace limit is approximately 10 GB because it shares storage with the Durable Object.

The container-side filesystem mirror is held in memory. Cloudflare explicitly recommends agent-scale workspaces rather than full monorepos.

### Heavy I/O

FUSE is not free.

Cloudflare's published benchmark for a full `npm install` of 854 packages / 36,675 files reports approximately:

```text
tmpfs:           34.3 s
ext4:            63.9 s
computerd FUSE: 124.7 s
```

That is roughly 2x slower than ext4 for this workload. Metadata-heavy operations perform much better, but dependency extraction and large sequential I/O need to be measured with our workloads.

### Hibernation is not shipped yet

The lifecycle design describes Durable Object hibernation, but the current container backend still uses a non-hibernating WebSocket path. The repository notes that an open workspace currently keeps the Durable Object warm for the life of that connection.

We should therefore not model current Computer economics as fully scale-to-zero at the Durable Object layer.

### Recovery gaps

The container filesystem is transient. Durable Object SQLite is the source of truth.

The documented lifecycle says reconnect/reconciliation is designed around revision watermarks, but transparent reconnect and some durability behavior remain ongoing work. In-flight execution is not itself durable across every failure mode.

### Language/runtime coupling

ORTYO is currently Rust-first. `@cloudflare/computer` is a TypeScript/Workers library.

A production integration should therefore live behind a service/provider boundary rather than forcing Cloudflare-specific TypeScript concepts into ORTYO core crates.

## Scope risk

Computer makes it tempting to turn ORTYO into a generic cloud IDE, coding-agent host, or Daytona-style workspace platform.

That would be scope drift.

A sandbox belongs in ORTYO only when it strengthens the same lifecycle:

```text
run isolated software
  -> force external interactions through a Boundary
  -> capture canonical evidence
  -> replay it
  -> assert Contracts/Scenarios
```

If a feature is useful only as a generic remote shell, it does not automatically belong in ORTYO.

## Proposed vertical slice

Do not adopt Computer as a production dependency yet.

Build one narrow proof outside the core domain:

```text
1. create one Cloudflare Computer workspace
2. seed a small repository/fixture
3. mutate/read files through the Worker backend
4. execute one Linux-only command through the Container backend
5. prove both backends observe the same workspace state
6. enforce default-deny egress, then allow one explicit destination
7. restart/reconnect the container and prove committed workspace files survive
8. route one permitted HTTP call through an ORTYO Boundary
9. prove the call becomes canonical Interaction evidence
10. record latency, CPU/memory, filesystem size, and I/O timings
```

The experiment is successful only if ORTYO remains the authority for policy/evidence while Computer remains replaceable execution infrastructure.

## Adoption gates

Before creating a production `CloudflareComputerProvider`, require evidence for all of these:

- provider-neutral ORTYO API with no Cloudflare types in canonical domain objects
- explicit separation from the existing HTTP-only `ExecutionProvider`
- default-deny network policy
- secret injection without exposing raw values to model-visible evidence
- deterministic cleanup/revocation
- recovery after container restart or dropped backend session
- bounded filesystem/resource consumption
- acceptable dependency-install and test performance for our target repositories
- ability to correlate execution with ORTYO Session / Interaction evidence
- documented behavior when Computer preview APIs change

## Current conclusion

`@cloudflare/computer` is one of the best current candidates for an ORTYO sandbox runtime experiment because it already separates durable workspace state from interchangeable execution backends and exposes controls that align with agent workloads.

It should remain:

```text
candidate provider, not ORTYO primitive
experimental, not production dependency
execution substrate, not source of truth for ORTYO evidence
```

The next useful step is the vertical slice above. A successful slice can justify an RFC for a provider-neutral `SandboxRuntimeProvider`. A failed slice costs us only an experiment and does not disturb `EXEC1` or the canonical model.

## Sources reviewed

- Cloudflare announcement: https://blog.cloudflare.com/cloudflare-computer/
- Cloudflare changelog: https://developers.cloudflare.com/changelog/post/2026-08-03-cloudflare-computer/
- Cloudflare Computer repository: https://github.com/cloudflare/computer
- Computer package overview: https://github.com/cloudflare/computer/blob/main/packages/computer/README.md
- Computer lifecycle: https://github.com/cloudflare/computer/blob/main/docs/11_lifecycle.md
- Computer performance: https://github.com/cloudflare/computer/blob/main/docs/19_performance.md
- Cloudflare Containers sandbox update and snapshots: https://blog.cloudflare.com/faster-agent-sandboxes/
- Cloudflare Containers egress controls: https://github.com/cloudflare/containers/blob/main/docs/egress.md
