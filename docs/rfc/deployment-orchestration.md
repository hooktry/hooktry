# ORCH1 - Deployment Orchestration

Status: accepted architecture rule  
Checked: 2026-10-01

## Decision

GitHub is the source repository for HOOKTRY and GitHub Actions is the canonical CI, release, and deployment orchestrator.

Infrastructure platforms are deployment providers, not release orchestrators for HOOKTRY.

~~~text
                         GitHub
                            |
                      source of truth
                            |
                      GitHub Actions
                            |
                canonical release/deploy control
                            |
          +-----------------+------------------+
          |                 |                  |
          v                 v                  v
     Cloudflare         Namespace         Container/BYOC
     adapter            adapter           adapter
          |                 |                  |
   Wrangler / API      provider API       OCI / Helm / SSH
~~~

This preserves one release control plane while allowing deployment targets to change independently.

## Terminology

### Deployment orchestrator

The system that decides:

- which commit or release is being deployed
- which checks must pass first
- which deployment profile is targeted
- which provider-specific adapter is invoked
- which production acceptance checks must pass
- which Git SHA/version is recorded as deployed

For HOOKTRY this is GitHub Actions.

### Deployment provider

Infrastructure that hosts or executes a deployment profile.

Examples:

- Cloudflare
- Namespace
- AWS
- Render
- Kubernetes
- Docker/OCI
- customer-owned BYOC infrastructure
- a future HOOKTRY-operated compute platform

Provider-specific tooling belongs below the orchestration boundary.

## Current mapping

~~~text
GitHub Actions
    |
    +-- deploy-cloudflare.yml
            |
            +-- Wrangler
            +-- Workers
            +-- D1
            +-- R2
            +-- Durable Objects
~~~

Cloudflare is the first production deployment provider. It is not the owner of the HOOKTRY release process.

The current Deploy Cloudflare workflow is therefore intentionally located under .github/workflows/.

## Why this boundary exists

HOOKTRY is more than one hosted Worker.

A release may eventually produce and coordinate:

~~~text
same Git SHA / release
    |
    +-- Rust CLI binary
    +-- macOS artifact / Homebrew
    +-- Linux artifact
    +-- Windows artifact
    +-- Docker/OCI image
    +-- MCP surface
    +-- managed Cloudflare runtime
    +-- future Namespace runtime
    +-- optional desktop/Tauri shell
~~~

A provider-owned build system can be excellent at deploying that provider's resources, but it cannot naturally become the authoritative release controller for all HOOKTRY surfaces and deployment profiles.

The canonical orchestration layer therefore remains provider-neutral.

## Provider-specific workflows are allowed

Provider-neutral orchestration does not mean pretending providers are identical.

Do not create one giant lowest-common-denominator deployment script.

It is correct to have provider-specific workflows or reusable jobs such as:

~~~text
deploy-cloudflare.yml
deploy-namespace.yml
deploy-container.yml
deploy-self-hosted.yml
~~~

Each may use the native provider toolchain:

- Cloudflare -> Wrangler/API
- Namespace -> Namespace API/CLI
- Kubernetes -> Helm/kubectl
- OCI -> Docker/BuildKit/registry tooling

The invariant is that GitHub Actions invokes and verifies them.

When a second real provider exists, a thin top-level dispatcher may be introduced:

~~~text
Deploy HOOKTRY
    target=cloudflare
    target=namespace
    target=container
~~~

Do not invent this dispatcher before a second real deployment target requires it.

## Build and provenance rule

Deployments must remain traceable to a Git commit or immutable release artifact.

Prefer:

~~~text
test once
identify Git SHA
build immutable artifact where applicable
deploy provider profile
run provider acceptance
record the same SHA/version
~~~

Provider-generated identifiers may supplement the HOOKTRY Git SHA but must not replace it as release provenance.

A production incident should be answerable with:

~~~text
Which HOOKTRY Git SHA is running?
Which deployment profile?
Which provider deployment/version corresponds to it?
~~~

For the Cloudflare profile, every Worker deployment receives the immutable Git SHA as `HOOKTRY_RELEASE_SHA`. The public `/healthz` response reports that revision. Production acceptance must observe the expected revision consistently before exercising the realtime Hook flow; a provider deployment command returning is not by itself sufficient evidence that the expected release is serving the acceptance path.

## Provider-native CI/CD

Provider-native CI/CD systems such as Cloudflare Builds are not forbidden.

They may be used later for capabilities that provide clear incremental value, for example:

- disposable experiments
- provider-specific previews
- development environments
- diagnostics that are difficult to reproduce externally

They are not the canonical production deployment path unless a future architecture decision explicitly changes this rule.

Avoid running two independent production deployment authorities for the same profile.

## Secrets and authority

Provider credentials belong to the provider deployment adapter and must be scoped to the deployment lifecycle phase.

Cloudflare uses two distinct authorities:

~~~text
bootstrap authority
    |
    | short-lived / Admin where resource creation requires it
    v
create D1 / R2 / Worker
    |
    v
ordinary deployment authority
    |
    | long-lived / Editor / existing HOOKTRY resources only
    v
migrate -> deploy -> acceptance
~~~

The GitHub `production` environment stores the ordinary deployment authority:

~~~text
secret: CLOUDFLARE_API_TOKEN
secret: HOOKTRY_CLAIM_INTERNAL_TOKEN
variable: CLOUDFLARE_ACCOUNT_ID
~~~

A privileged bootstrap credential, when required, uses the separate `CLOUDFLARE_BOOTSTRAP_API_TOKEN` secret and should be removed after bootstrap.

Ordinary deployment must not silently recreate missing infrastructure. Missing provider resources are an infrastructure/bootstrap failure, not permission to escalate a normal deploy into provisioning.

Cloudflare API token policy changes and token-secret rotation are separate operations. Prefer reducing an existing token's permissions without rolling its secret when the credential value itself has not been exposed.

Those credentials authorize the Cloudflare deployment profile. They do not make Cloudflare the HOOKTRY release authority.

Future provider credentials should remain similarly scoped to their adapter.

## Relationship to PORTS1

PORTS1 separates application/runtime semantics from infrastructure adapters.

ORCH1 applies the same rule one level above runtime:

~~~text
PORTS1:
domain/application != infrastructure provider

ORCH1:
release/deployment orchestration != infrastructure provider
~~~

Together they preserve both runtime portability and operational portability.

## Relationship to self-host and BYOC

Self-hosted and BYOC distributions remain first-class deployment profiles.

GitHub Actions may publish artifacts that customers deploy themselves. HOOKTRY does not need to retain operational authority over customer-owned infrastructure for those profiles.

Examples:

~~~text
GitHub release -> hooktry binary -> user machine

GitHub release -> OCI image -> customer Kubernetes

HOOKTRY Cloud control plane -> enrolled customer compute
~~~

The release provenance still originates from the HOOKTRY repository even when the final deployment action occurs outside HOOKTRY-operated infrastructure.

## Non-goals

ORCH1 does not:

- require every deployment to use the same provider API
- require self-hosted users to grant HOOKTRY access to their infrastructure
- prohibit Cloudflare Builds or other provider-native tooling
- introduce a generic multi-provider deployment framework before it is needed
- make GitHub Actions part of the HOOKTRY runtime
- make Cloudflare, Namespace, AWS, or another provider part of the HOOKTRY domain model
