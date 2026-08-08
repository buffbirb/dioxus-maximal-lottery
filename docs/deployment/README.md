# Deployment

The project is deployed to [Render](https://render.com/) using container images built with Nix. Database schema changes are managed through [Supabase](https://supabase.com/) migrations and applied before the application is rolled out.

## Philosophy

- **Declarative infrastructure**: Services, environment variables, and runtime dependencies are described in version-controlled configuration rather than configured by hand in hosting dashboards.
- **Reproducible builds**: The web bundle and container image are produced inside the same Nix-defined environment used for local development, minimizing "works on my machine" drift between CI and production.
- **Immutable artifacts**: Each time the image is built, it is pushed to a registry and referenced by tag. Promotion between environments happens by retagging or referencing an existing image, not by rebuilding.
- **Migrations before deploys**: Database migrations run as part of the deployment process and must succeed before the new application version is triggered to start. Migrations are assumed to be backward-compatible; if a deploy fails after migrations have already run, the database may be ahead of the running code. Database rollback is not handled by this pipeline.
- **Expand/contract migrations**: Because the old code keeps running against the migrated schema until the rollout completes, each release's migrations must be expand-only: add tables and columns (nullable or with defaults), and never drop, rename, or retype a column in the same release that stops using it. Destructive "contract" changes ship in a later release once no deployed code depends on the old shape.

## Environments

### Development

Every successful change on `main` produces a new development image and deploys it automatically to the development environment. Manual deployments from feature branches are a single step for testing changes before merging: run the **Deploy dev** workflow and pick a branch (or type any ref) — the image is built, pushed, migrated, deployed, and promoted in one run.

### Production

Production releases are created by making a GitHub release, which produces a version tag. That tagged image is deployed manually through a workflow, then promoted to the stable production tag so the Render blueprint continues to reference a single, predictable image tag.

## Pipeline responsibilities

- **Build image**: compile the release web bundle, wrap it in a layered container image, and push the result to the container registry.
- **Verify image**: fail fast if the target image tag does not exist in the registry, before any migration runs.
- **Migrate**: apply pending Supabase schema changes to the target environment.
- **Configure**: synchronize environment variables from GitHub to the target Render service.
- **Deploy**: trigger a Render deployment with the image URL, wait for the rollout to reach the live state, and only then retag the image as the environment's stable tag.

## Rollbacks

To roll back an environment to an older image, run **Deploy dev** (with the old ref) or **Deploy prd** (with the old tag) and uncheck `run_migrations`. The deploy then skips the database entirely and only re-points Render at the older image. This is safe because migrations are expand-only: the newer schema remains compatible with the older code.

## Observability

The server exports OpenTelemetry traces over OTLP (HTTP/protobuf) to Grafana Cloud, enabled by the presence of `OTEL_EXPORTER_OTLP_ENDPOINT`. See [render.yaml](/render.yaml) for relevant variables.

## Required configuration

Configured by hand in the GitHub environment matching the deploy target.

### Secrets

| Variable | Description |
|---|---|
| `BASIC_AUTH_PASSWORD` | HTTP basic auth password, synced to non-production environments only |
| `BASIC_AUTH_USERNAME` | HTTP basic auth username, synced to non-production environments only |
| `DATABASE_URL` | Postgres connection string synced to the Render service |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OpenTelemetry OTLP HTTP endpoint synced to the Render service |
| `OTEL_EXPORTER_OTLP_HEADERS` | OpenTelemetry exporter authentication headers synced to the Render service |
| `RENDER_API_KEY` | Render API authentication for syncing environment variables |
| `RENDER_DEPLOY_HOOK` | Render deploy hook URL for triggering deployments |
| `RENDER_SERVICE_ID` | Render service identifier used when syncing environment variables |
| `SUPABASE_ACCESS_TOKEN` | Supabase CLI authentication for linking and pushing migrations |

### Variables

| Variable | Description |
|---|---|
| `SUPABASE_PROJECT_ID` | Supabase project reference used when linking |

## Optional configuration

| Variable | Description |
|---|---|
| `PUBLIC_BASE_URL` | Pins the origin (`https://host[:port]`, no path) that server-rendered share links are built on. Unset on Render, where the proxy preserves `Host` and sets `X-Forwarded-Proto`; set it only where those headers cannot be trusted, such as behind a proxy chain that rewrites them. A value that is not a bare origin is logged and ignored. |
