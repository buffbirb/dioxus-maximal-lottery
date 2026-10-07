# Database

The application uses PostgreSQL via [Supabase](https://supabase.com).

## Architecture

```
supabase/migrations/*.sql  ← single source of truth
       │
       ├── CI:           sqlx migrate run against GitHub Actions postgres
       │                  service (ephemeral, thrown away after job)
       │
       ├── Local:        `make go` (mise + process-compose) or devenv's
       │                  services.postgres; both run sqlx migrate run on start
       │                  (or `supabase db start` for Supabase CLI commands)
       │
       └── Production:   GitHub Actions + Supabase CLI
                          (deploy workflow runs supabase db push on merge to main)
```

## Local development

The local toolchain is [mise](https://mise.jdx.dev). `mise.toml` pins Rust, `dx`,
`sqlx-cli`, and Postgres 17, and sets `DATABASE_URL` and `PGDATA`. Once per clone:

```bash
mise trust
make install
```

Then activate it in your shell, either with `mise activate` in your shell rc or per
session with `eval "$(mise env)"`.

### GitHub sign-in credentials

The server refuses to start without `OAUTH_GITHUB_CLIENT_ID` and
`OAUTH_GITHUB_CLIENT_SECRET`, locally too. Use the dev GitHub App's values (see
[docs/deployment](/docs/deployment/README.md#github-sign-in)) in a git-ignored
`mise.local.toml` next to `mise.toml`, which mise layers on top:

```toml
[env]
OAUTH_GITHUB_CLIENT_ID = "..."
OAUTH_GITHUB_CLIENT_SECRET = "..."
```

The dev app's local callback is `http://127.0.0.1:8080/login/github/callback`, so
browse the app at `127.0.0.1:8080` (not `localhost`) when signing in. In the sbx
container, launch with `SBX_PORTS=8080` and run
`PUBLIC_BASE_URL=http://127.0.0.1:8080 WEB_HOST=0.0.0.0 make go`, so the callback
URL stays on the registered loopback address.

### Running

```bash
make go
```

starts Postgres, applies pending migrations with `sqlx migrate run`, and runs
`dx serve`, all through process-compose. Against an already running Postgres:

- `make db-info` — show migration status
- `make db-migrate` — apply pending migrations
- `make db-reset` — drop and recreate the public schema (run `make db-migrate` after)

The data directory is `$PGDATA`, under `.local/`. A data directory created by the
old makey `make go` applied migrations with `psql -f` and has no
`_sqlx_migrations` table, so `sqlx migrate run` would replay the first migrations
and fail. Start from a fresh `$PGDATA` rather than moving an old one in.

### devenv

`devenv up` also applies pending migrations automatically. Check migration status:

```bash
devenv tasks run db:info
```

For a full reset:

```bash
devenv tasks run db:reset
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
