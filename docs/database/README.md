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
brings the matching orchestrator and dashboard. The command prints the
dashboard's address, and the dashboard links to what each resource serves:

| Resource | Does |
| --- | --- |
| `lottery` | The app's database |
| `migrate` | Applies the migrations `_sqlx_migrations` hasn't recorded, then exits. Restart it to apply a migration you just added. |
| `postgres` | Postgres 17. A volume keeps its data between runs, and its **Manage (pgweb)** link opens a web UI for browsing it. |
| `prepare-queries` | Starts only when you start it: regenerates `.sqlx/` from the database |
| `reset-database` | Starts only when you start it: drops and recreates the database, then applies every migration |
| `web` | `dx serve` with hot reload, once `migrate` has finished |

Aspire picks every port afresh on each run, so open the app from the dashboard.
Worktrees can each run their AppHost at the same time, each with its own ports,
containers and Postgres volume.

sqlx checks queries at compile time: under the AppHost against its database, and
everywhere else, such as in your editor, against the query data in `.sqlx/`.
After changing a query or the schema, start `prepare-queries` and commit what it
writes. CI fails while `.sqlx/` is out of date.

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

Locally, sign-in goes through an OAuth App rather than the dev GitHub App: a
GitHub App's callback has to match the app's port exactly, while an OAuth App's
loopback callback accepts any port. Create one under **Settings → Developer
settings → OAuth Apps**, with `http://127.0.0.1/login/github/callback` as its
callback, generate a client secret, and store both values in the AppHost's user
secrets:

```bash
dotnet user-secrets set --project apphost Parameters:oauth-github-client-id "..."
dotnet user-secrets set --project apphost Parameters:oauth-github-client-secret "..."
```

Browse the app at the `127.0.0.1` address the dashboard links to, not
`localhost`, so the callback returns to the same origin. Cookies aren't scoped
by port, so apps from several worktrees open in one browser share them, and
signing in to one signs you out of the others. Use a browser profile per
worktree to stay signed in to each.

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
any Rust build steps, so that sqlx compile-time query checking has a live schema
to validate against, and CI checks `.sqlx/` against it.
