# Database

The application uses PostgreSQL via [Supabase](https://supabase.com).

## Architecture

```
supabase/migrations/*.sql  ← single source of truth
       │
       ├── CI:           psql against GitHub Actions postgres service
       │                  (ephemeral, thrown away after job)
       │
       ├── Local:        devenv services.postgres (auto-started by `devenv up`)
       │                  or `supabase db start` for Supabase CLI commands
       │
       └── Production:   GitHub Actions + Supabase CLI
                          (deploy workflow runs supabase db push on merge to main)
```

## Local development

Start PostgreSQL detached, apply migrations, then stop:

```bash
devenv up postgres -d
psql "$DATABASE_URL" -f supabase/migrations/<file>
devenv processes down
```

The database is accessible at `DATABASE_URL` while the postgres process is
running.

For a full reset:

```bash
supabase db reset
```

### Supabase CLI

The [Supabase CLI](https://supabase.com/docs/guides/local-development/cli/getting-started)
is installed in the devenv shell for administrative tasks:

- `supabase db diff` — generate a new migration from schema changes
- `supabase db push` — apply pending migrations to a linked remote project
- `supabase db pull` — pull a remote schema into a local migration file
- `supabase db reset` — destroy and recreate the local database from migrations

These commands manage their own database container on a separate port (default 54322).
They do not interfere with the devenv-managed Postgres instance.

## CI

GitHub Actions uses a postgres service container. Migrations are applied
with `psql` before any Rust build steps so that sqlx compile-time query checking
has a live schema to validate against.
