# Deployment

The project is deployed to [Render](https://render.com/) using container images built with Nix. Database schema changes are managed through [Supabase](https://supabase.com/) migrations and applied before the application is rolled out.

## Philosophy

- **Declarative infrastructure**: Services, environment variables, and runtime dependencies are described in version-controlled configuration rather than configured by hand in hosting dashboards.
- **Reproducible builds**: The web bundle and container image are produced inside the same Nix-defined environment used for local development, minimizing "works on my machine" drift between CI and production.
- **Immutable artifacts**: Each time the image is built, it is pushed to a registry and referenced by tag. Promotion between environments happens by retagging or referencing an existing image, not by rebuilding.
- **Migrations before deploys**: Database migrations run as part of the deployment process and must succeed before the new application version is triggered to start. Migrations are assumed to be backward-compatible; if a deploy fails after migrations have already run, the database may be ahead of the running code. Database rollback is not handled by this pipeline.

## Environments

### Development

Every successful change on `main` produces a new development image and deploys it automatically to the development environment. Manual deployments from feature branches are also supported for testing changes before merging.

### Production

Production releases are created by making a GitHub release, which produces a version tag. That tagged image is deployed manually through a workflow, then promoted to the stable production tag so the Render blueprint continues to reference a single, predictable image tag.

## Pipeline responsibilities

- **Build image**: compile the release web bundle, wrap it in a layered container image, and push the result to the container registry.
- **Migrate**: apply pending Supabase schema changes to the target environment.
- **Configure**: synchronize environment variables from GitHub to the target Render service.
- **Deploy**: trigger a Render deployment with the image URL and retag the image as the environment's stable tag after a successful rollout.

## Observability

The server exports OpenTelemetry traces over OTLP (HTTP/protobuf) to Grafana Cloud, enabled by the presence of `OTEL_EXPORTER_OTLP_ENDPOINT`. Relevant variables are declared in the Render blueprint.

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
