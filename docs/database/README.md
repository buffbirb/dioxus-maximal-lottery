# Database

The application uses PostgreSQL via [Supabase](https://supabase.com).

## Architecture

```
supabase/migrations/*.sql  ← single source of truth
       │
       ├── CI:           sqlx migrate run against GitHub Actions postgres
       │                  service (ephemeral, thrown away after job)
       │
       ├── Local:        the Aspire AppHost's Postgres container or `make go`
       │                  (mise + process-compose); both run sqlx migrate run
       │                  on start
       │                  (or `supabase db start` for Supabase CLI commands)
       │
       └── Production:   GitHub Actions + Supabase CLI
                          (deploy workflow runs supabase db push on merge to main)
```

## Local development

The [Aspire](https://aspire.dev) AppHost in `apphost/` runs the local stack:
Postgres 17 in a container, the migrations, and `dx serve`, each waiting for
what it depends on, with a dashboard for their state, logs and traces. It needs:

- `dx` and `sqlx-cli` on your PATH (see
  [docs/development](/docs/development/README.md)).
- Podman, the container runtime `apphost/appsettings.json` selects; set
  `ASPIRE_CONTAINER_RUNTIME` to use another. On macOS, create its VM once with
  `podman machine init`, then `podman machine start` it when needed.
- The .NET SDK `global.json` pins.

From the repository root:

```bash
dotnet run --project apphost
```

The first run downloads the Aspire CLI that `Directory.Packages.props` pins,
which brings the matching orchestrator and dashboard. The command prints the
dashboard's address, and the dashboard links to what each resource serves:

| Resource | Does |
| --- | --- |
| `lottery` | The app's database |
| `migrate` | Applies the migrations `_sqlx_migrations` hasn't recorded, then exits. Restart it to apply a migration you just added. |
| `postgres` | Postgres 17. A volume keeps its data between runs, and its **Manage (pgweb)** link opens a web UI for browsing it. |
| `prepare-queries` | Starts only when you start it: regenerates `.sqlx/` from the database (see [Query checking](#query-checking)) |
| `reset-database` | Starts only when you start it: drops and recreates the database, then applies every migration |
| `web` | `dx serve` with hot reload, once `migrate` has finished |

Aspire picks every port afresh on each run, so open the app from the dashboard.
Worktrees can each run their AppHost at the same time, each with its own ports,
containers and Postgres volume.

The AppHost points the server's OpenTelemetry exporter at the dashboard, so its
traces show up under **Traces**, beside every resource's console output. Its
launch profile serves the dashboard over plain HTTP, so there's no development
certificate to create or trust.

### Docker sandboxes

A Docker sandbox (`sbx`) has its own Docker daemon, but forwards only the ports
published to it, and only to servers listening on every interface. Choose a
port for `web`, publish it with `sbx port publish <sandbox> 8080:8080` (or
`--publish 8080:8080` on `sbx run`), and run the AppHost with:

```bash
ASPIRE_CONTAINER_RUNTIME=docker WEB_HOST=0.0.0.0 WEB_PORT=8080 dotnet run --project apphost
```

The host's browser then reaches the app on the same `127.0.0.1:8080` address
that the sign-in callback uses.

### GitHub sign-in credentials

The server refuses to start without `OAUTH_GITHUB_CLIENT_ID` and
`OAUTH_GITHUB_CLIENT_SECRET`, locally too. The AppHost passes them from its
`oauth-github-client-id` and `oauth-github-client-secret` parameters, and the
dashboard asks for any that aren't set.

Under the AppHost, sign-in goes through an OAuth App rather than the dev GitHub
App: a GitHub App's callback has to match the app's port exactly, while an
OAuth App's loopback callback accepts any port. Create one under **Settings →
Developer settings → OAuth Apps**, with `http://127.0.0.1/login/github/callback`
as its callback, generate a client secret, and store both values in the
AppHost's user secrets:

```bash
dotnet user-secrets set --project apphost Parameters:oauth-github-client-id "..."
dotnet user-secrets set --project apphost Parameters:oauth-github-client-secret "..."
```

Browse the app at the `127.0.0.1` address the dashboard links to, not
`localhost`, so the callback returns to the same origin. Cookies aren't scoped
by port, so apps from several worktrees open in one browser share them, and
signing in to one signs you out of the others. Use a browser profile per
worktree to stay signed in to each.

### mise and `make go`

`make go` runs the stack on fixed ports, with the toolchain from
[mise](https://mise.jdx.dev). `mise.toml` pins Rust, `dx`, `sqlx-cli`, and
Postgres 17, and sets `DATABASE_URL` and `PGDATA`. Once per clone:

```bash
mise trust
make install
```

Then activate it in your shell, either with `mise activate` in your shell rc or per
session with `eval "$(mise env)"`.

#### Sign-in credentials

Use the dev GitHub App's values (see
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

#### Running

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

### Supabase CLI

The [Supabase CLI](https://supabase.com/docs/guides/local-development/cli/getting-started),
which `tools/supabase` pins, is for administrative tasks:

- `supabase db diff` — generate a new migration from schema changes
- `supabase db pull` — pull a remote schema into a local migration file
- `supabase db push` — apply pending migrations to a linked remote project
- `supabase db reset` — destroy and recreate the local database from migrations

These commands manage their own database container on a separate port (default
54322). They do not interfere with the AppHost's Postgres or `make go`'s.

## Query checking

sqlx checks every query at compile time: against the database `DATABASE_URL`
names, which the AppHost gives `dx serve` and mise sets to `make go`'s
Postgres, or, when it's unset, against the query data committed in `.sqlx/`. So
a build needs no database outside those, such as in an editor, and
`SQLX_OFFLINE=true` makes one in a mise shell use `.sqlx/` too.

After changing a query or the schema, regenerate `.sqlx/` against a migrated
database and commit it: start the AppHost's `prepare-queries`, or run

```bash
cargo clean --package api
cargo sqlx prepare --workspace -- --all-features --all-targets
```

The clean matters: sqlx-cli rebuilds only the crates that use sqlx under
default features, and `api` uses it only under `server`. Without the clean, an
already built `api` isn't checked again, so `.sqlx/` stays as it was, and
`--check` passes whatever it holds.

## CI

The Rust job applies the migrations to a Postgres service container, checks
`.sqlx/` against the result, and compiles against it, so it fails while `.sqlx/`
is out of date. Every other build compiles against `.sqlx/`.
