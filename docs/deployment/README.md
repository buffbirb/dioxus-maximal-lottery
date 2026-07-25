# Deployment

The project is deployed to [Render](https://render.com/) using container images built with Nix. Database schema changes are managed through [Supabase](https://supabase.com/) migrations and applied before the application is rolled out.

## Philosophy

- **Declarative infrastructure**: Services, environment variables, and runtime dependencies are described in version-controlled configuration rather than configured by hand in hosting dashboards.
- **Reproducible builds**: The web bundle and container image are produced inside the same Nix-defined environment used for local development, minimizing "works on my machine" drift between CI and production.
- **Immutable artifacts**: Each build produces a container image that is pushed to a registry and referenced by tag. Promotion between environments happens by retagging or referencing an existing image, not by rebuilding.
- **Migrations before deploys**: Database migrations run as part of the deployment process and must succeed before the new application version is triggered to start.

## Environments

### Development

Every successful change on `main` produces a new development image and deploys it automatically to the dev environment. The dev environment always reflects the latest merged state.

### Production

Production releases are created by making a GitHub release, which produces a version tag. That tagged image is deployed manually through a workflow, then promoted to the stable production tag so the Render blueprint continues to reference a single, predictable image tag.

## Pipeline responsibilities

- **Build**: compile the release web bundle and produce a layered container image.
- **Push**: publish the image to GHCR with tags derived from the branch or release tag.
- **Migrate**: apply pending Supabase migrations against the target environment.
- **Deploy**: trigger the Render deploy hook with the image URL that should be rolled out.
- **Promote**: for production, retag the release image as the stable production tag after a successful deploy.
