# ORCH1 - Deployment Orchestration

Status: accepted architecture rule  
Checked: 2026-10-01

## Decision

GitHub is the source repository for ORTYO and GitHub Actions is the canonical CI, release, and deployment orchestrator.

Infrastructure platforms are deployment providers, not release orchestrators for ORTYO.

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

For ORTYO this is GitHub Actions.

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
- a future ORTYO-operated compute platform

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

Cloudflare is the first production deployment provider. It is not the owner of the ORTYO release process.

The current Deploy Cloudflare workflow is therefore intentionally located under .github/workflows/.

## Why this boundary exists

ORTYO is more than one hosted Worker.

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

A provider-owned build system can be excellent at deploying that provider's resources, but it cannot naturally become the authoritative release controller for all ORTYO surfaces and deployment profiles.

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
Deploy ORTYO
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

Provider-generated identifiers may supplement the ORTYO Git SHA but must not replace it as release provenance.

A production incident should be answerable with:

~~~text
Which ORTYO Git SHA is running?
Which deployment profile?
Which provider deployment/version corresponds to it?
~~~

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

Provider credentials belong to the provider deployment adapter.

For example, the Cloudflare deployment workflow may consume:

~~~text
CLOUDFLARE_API_TOKEN
CLOUDFLARE_ACCOUNT_ID
ORTYO_CLAIM_INTERNAL_TOKEN
~~~

Those credentials authorize the Cloudflare deployment profile. They do not make Cloudflare the ORTYO release authority.

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

GitHub Actions may publish artifacts that customers deploy themselves. ORTYO does not need to retain operational authority over customer-owned infrastructure for those profiles.

Examples:

~~~text
GitHub release -> ortyo binary -> user machine

GitHub release -> OCI image -> customer Kubernetes

ORTYO Cloud control plane -> enrolled customer compute
~~~

The release provenance still originates from the ORTYO repository even when the final deployment action occurs outside ORTYO-operated infrastructure.

## Non-goals

ORCH1 does not:

- require every deployment to use the same provider API
- require self-hosted users to grant ORTYO access to their infrastructure
- prohibit Cloudflare Builds or other provider-native tooling
- introduce a generic multi-provider deployment framework before it is needed
- make GitHub Actions part of the ORTYO runtime
- make Cloudflare, Namespace, AWS, or another provider part of the ORTYO domain model
