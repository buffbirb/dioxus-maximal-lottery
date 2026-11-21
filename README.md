# Maximal Lottery

A [Dioxus](https://github.com/dioxuslabs/dioxus) fullstack application for ranked-choice polls.

## Features

- **CI**: GitHub Actions jobs that build, lint and test natively from the pinned toolchain, in parallel (see [docs/development](docs/development))
- **Code Quality**: [prek](https://prek.j178.dev) runs the git hooks in `.pre-commit-config.yaml` to enforce a clean and consistent coding style
- **Deployment**: Render deployment pipeline using container images built from the Dockerfile and Supabase migrations (see [docs/deployment](docs/deployment))
- **Development Environment**: [devenv](https://github.com/cachix/devenv) is included for an opt-in batteries-included experience with [OpenTelemetry Collector](https://github.com/open-telemetry/opentelemetry-collector-contrib) and [Clickhouse](https://github.com/ClickHouse/ClickHouse)
- **Local Stack**: an [Aspire](https://aspire.dev) AppHost runs Postgres, the migrations and the dev server, with a dashboard for their logs and the server's traces (see [docs/database](docs/database))

## Note

In a Docker sandbox, run the AppHost as [docs/database](docs/database/README.md#docker-sandboxes) describes, so the host can reach the app.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
