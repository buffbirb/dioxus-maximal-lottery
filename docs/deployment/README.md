# Deployment

The project is deployed to [Render](https://render.com/) using container images built with Nix. Database schema changes are managed through [Supabase](https://supabase.com/) migrations and applied before the application is rolled out.

## Philosophy

- **Declarative infrastructure**: Services, environment variables, and runtime dependencies are described in version-controlled configuration rather than configured by hand in hosting dashboards.
- **Reproducible builds**: The web bundle and container image are produced inside the same Nix-defined environment used for local development, minimizing "works on my machine" drift between CI and production.
- **Immutable artifacts**: Each build produces a container image that is pushed to a registry and referenced by tag. Production deployments reference a stable `:prd` tag that is only updated after a successful release deploy.
- **Migrations before deploys**: Database migrations run as part of the deployment process and must succeed before the new application version is triggered to start.

## Environments

### Development

All development happens locally via [devenv](https://devenv.sh/) (`devenv up`). There is no persistent public development deployment, because Render's free web services are always publicly accessible on the internet and cannot be restricted to internal use.

### Production

Production releases are created by making a GitHub release, which produces a version tag. That tagged image is deployed manually through the `Deploy prd` workflow, then promoted to the stable `:prd` tag so the Render blueprint continues to reference a single, predictable image tag.

## Pipeline responsibilities

- **Build**: compile the release web bundle and produce a layered container image.
- **Push**: publish the image to GHCR with tags derived from the branch or release tag.
- **Migrate**: apply pending Supabase migrations against the production database.
- **Deploy**: trigger the Render deploy hook with the image URL that should be rolled out.
- **Promote**: retag the release image as the stable `:prd` tag after a successful deploy.
