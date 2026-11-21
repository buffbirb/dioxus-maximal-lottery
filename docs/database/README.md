# Database

The application uses PostgreSQL via [Supabase](https://supabase.com).

## Architecture

```
supabase/migrations/*.sql  ← single source of truth
       │
       ├── CI:           sqlx migrate run against GitHub Actions postgres
       │                  service (ephemeral, thrown away after job)
       │
       ├── Local:        the Aspire AppHost's Postgres container; its migrate
       │                  resource runs sqlx migrate run on every start
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

The first run downloads the Aspire CLI that `apphost/AppHost.csproj` pins, which
brings the matching orchestrator and dashboard. Then the dashboard opens:

| Resource | Does |
| --- | --- |
| `lottery` | The app's database |
| `migrate` | Applies the migrations `_sqlx_migrations` hasn't recorded, then exits. Restart it to apply a migration you just added. |
| `postgres` | Postgres 17 on `127.0.0.1:5432`, user and password `postgres`. A volume keeps its data between runs, and its **Manage (pgweb)** link opens a web UI for browsing it. |
| `reset-database` | Starts only when you start it: drops and recreates the database, then applies every migration |
| `web` | `dx serve` with hot reload on `http://127.0.0.1:8080`, once `migrate` has finished |

`.cargo/config.toml` points `DATABASE_URL` at the same database, so `cargo`,
clippy and rust-analyzer can check sqlx queries while the AppHost runs.

The AppHost points the server's OpenTelemetry exporter at the dashboard, so its
traces show up under **Traces**, beside every resource's console output. Its
launch profile serves the dashboard over plain HTTP, so there's no development
certificate to create or trust.

### Docker sandboxes

A Docker sandbox (`sbx`) has its own Docker daemon, and forwards a port to the
host only from a server listening on every interface. Launch it with
`SBX_PORTS=8080`, then run the AppHost with:

```bash
ASPIRE_CONTAINER_RUNTIME=docker WEB_HOST=0.0.0.0 dotnet run --project apphost
```

The sign-in callback stays on `127.0.0.1:8080`, which the host's browser reaches
through the forwarded port.

### GitHub sign-in credentials

The server refuses to start without `OAUTH_GITHUB_CLIENT_ID` and
`OAUTH_GITHUB_CLIENT_SECRET`, locally too. The AppHost passes them from its
`oauth-github-client-id` and `oauth-github-client-secret` parameters, and the
dashboard asks for any that aren't set. Use the dev GitHub App's values (see
[docs/deployment](/docs/deployment/README.md#github-sign-in)), stored in the
AppHost's user secrets:

```bash
dotnet user-secrets set --project apphost Parameters:oauth-github-client-id "..."
dotnet user-secrets set --project apphost Parameters:oauth-github-client-secret "..."
```

The dev app's local callback is `http://127.0.0.1:8080/login/github/callback`, so
browse the app at `127.0.0.1:8080` (not `localhost`) when signing in. The
dashboard links to that address.

### Supabase CLI

The [Supabase CLI](https://supabase.com/docs/guides/local-development/cli/getting-started),
which `tools/supabase` pins, is for administrative tasks:

- `supabase db diff` — generate a new migration from schema changes
- `supabase db pull` — pull a remote schema into a local migration file
- `supabase db push` — apply pending migrations to a linked remote project
- `supabase db reset` — destroy and recreate the local database from migrations

These commands manage their own database container on a separate port (default
54322). They do not interfere with the AppHost's Postgres.

## CI

GitHub Actions uses a postgres service container. Migrations are applied before
any Rust build steps so that sqlx compile-time query checking has a live schema
to validate against.
