# Maximal Lottery

A [Dioxus](https://github.com/dioxuslabs/dioxus) fullstack application for ranked-choice polls.

## Features

- **CI**: GitHub Actions jobs that build, lint and test natively from the pinned toolchain, in parallel (see [docs/development](docs/development))
- **Code Quality**: [prek](https://prek.j178.dev) runs the git hooks in `.pre-commit-config.yaml` to enforce a clean and consistent coding style
- **Deployment**: Render deployment pipeline using Nix-built container images and Supabase migrations (see [docs/deployment](docs/deployment))
- **Development Environment**: [devenv](https://github.com/cachix/devenv) is included for an opt-in batteries-included experience with [OpenTelemetry Collector](https://github.com/open-telemetry/opentelemetry-collector-contrib) and [Clickhouse](https://github.com/ClickHouse/ClickHouse)

## Note

In sandbox, run `dx serve --addr 0.0.0.0 --port 8080` to forward to host

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
