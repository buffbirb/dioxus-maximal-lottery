# Database

The application uses PostgreSQL via [Supabase](https://supabase.com).

## Architecture

```
supabase/migrations/*.sql  ← single source of truth
       │
       ├── CI:           sqlx migrate run against GitHub Actions postgres
       │                  service (ephemeral, thrown away after job)
       │
       ├── Local:        devenv services.postgres; a db:migrate task applies
       │                  pending migrations automatically on `devenv up`
       │                  (or `supabase db start` for Supabase CLI commands)
       │
       └── Production:   GitHub Actions + Supabase CLI
                          (deploy workflow runs supabase db push on merge to main)
```

## Local development

`devenv up` applies pending migrations automatically: the `db:migrate` task
runs `sqlx migrate run --source supabase/migrations` once Postgres is ready,
and the `web` process waits for it to succeed before building, so sqlx
compile-time query checks always see a migrated schema. A failed migration
blocks `web` from starting instead of surfacing as obscure build errors.

Check migration status:

```bash
sqlx migrate info --source supabase/migrations
```

Applied migrations are tracked in the `_sqlx_migrations` table of the
devenv-managed Postgres. This is independent of the Supabase CLI's tracking
for remote databases, so the two never interfere.

For a full reset of the local database, while the devenv Postgres is running:

```bash
psql "$DATABASE_URL" -c 'DROP SCHEMA public CASCADE; CREATE SCHEMA public;'
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

GitHub Actions uses a postgres service container. Migrations are applied before
any Rust build steps so that sqlx compile-time query checking has a live schema
to validate against.
