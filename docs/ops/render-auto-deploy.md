# Render automatic deploy acceptance

DWC: PROD/DEPLOY1.2 DEPLOY1 - GitHub -> Render commit-trigger proof

ORTYO production is expected to deploy from the `main` branch through the connected GitHub provider.

Acceptance criteria:

- Render service has automatic deploys enabled for commits.
- A merge to `main` creates a Render deploy without an API-triggered fallback.
- The Render deploy reports `trigger=commit`.
- The deploy commit equals the merged GitHub commit.
- `GET /healthz` reports the same revision through `RENDER_GIT_COMMIT`.

If no commit-triggered deploy appears, treat that as a GitHub provider/integration failure. Do not mask the failure by using a manual API deploy while testing this acceptance path.
