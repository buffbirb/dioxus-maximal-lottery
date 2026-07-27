# Deployment

The project is deployed to [Render](https://render.com/) using container images built with Nix. Database schema changes are managed through [Supabase](https://supabase.com/) migrations and applied before the application is rolled out.

## Philosophy

- **Declarative infrastructure**: Services, environment variables, and runtime dependencies are described in version-controlled configuration rather than configured by hand in hosting dashboards.
- **Reproducible builds**: The web bundle and container image are produced inside the same Nix-defined environment used for local development, minimizing "works on my machine" drift between CI and production.
- **Immutable artifacts**: Each run of the Build image workflow produces a container image that is pushed to a registry and referenced by tag. Promotion between environments happens by retagging or referencing an existing image, not by rebuilding.
- **Migrations before deploys**: Database migrations run as part of the deployment process and must succeed before the new application version is triggered to start. Migrations are assumed to be backward-compatible; if a deploy fails after migrations have already run, the database may be ahead of the running code. Database rollback is not handled by this pipeline.

## Environments

### Development

Every successful change on `main` produces a new development image and deploys it automatically to the dev environment. The dev environment always reflects the latest merged state.

### Production

Production releases are created by making a GitHub release, which produces a version tag. That tagged image is deployed manually through a workflow, then promoted to the stable production tag so the Render blueprint continues to reference a single, predictable image tag.

## Pipeline responsibilities

- **Bundle**: compile the release web bundle with `dx bundle`.
- **Build image**: wrap that bundle in a layered container image with `nix build -f image.nix`.
- **Push**: publish the image to GHCR tagged with the commit SHA, plus the version tag on release runs.
- **Migrate**: apply pending Supabase migrations against the target environment.
- **Sync**: push the environment's `sync: false` variables from GitHub secrets to the Render service via the Render API. Entries with a literal `value` in `render.yaml` land only on a blueprint sync.
- **Deploy**: trigger the environment's Render deploy hook with the image URL to roll out.
- **Promote**: retag the deployed image as `:dev` or `:prd` after a successful deploy. Runs for every environment.

## Observability

The server exports OpenTelemetry traces over OTLP (HTTP/protobuf) to Grafana Cloud, enabled by the presence of `OTEL_EXPORTER_OTLP_ENDPOINT`. Check `render.yaml` for relevant variables.

## Required configuration

Configured by hand in the GitHub environment matching the deploy target (`dev` or `prd`).

Secrets:

- `SUPABASE_ACCESS_TOKEN`: Supabase CLI auth for `db push`.
- `RENDER_API_KEY`, `RENDER_SERVICE_ID`: environment variable sync.
- `RENDER_DEPLOY_HOOK`: deploy trigger.
- `DATABASE_URL`, `OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_HEADERS`: synced to Render.
- `BASIC_AUTH_USERNAME`, `BASIC_AUTH_PASSWORD`: synced to Render except in `prd`.

Variables:

- `SUPABASE_PROJECT_ID`: passed to `supabase link`.
