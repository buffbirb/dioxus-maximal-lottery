# Dioxus Fullstack Template

A template for building a [Dioxus](https://github.com/dioxuslabs/dioxus) fullstack application.

## Features

- **Development Environment**: [devenv](https://github.com/cachix/devenv) is included for an opt-in batteries-included experience with [OpenTelemetry Collector](https://github.com/open-telemetry/opentelemetry-collector-contrib) and [Clickhouse](https://github.com/ClickHouse/ClickHouse)
- **Code Quality**: [Pre-commit](https://github.com/pre-commit/pre-commit) hooks are configured to enforce a clean and consistent coding style
- **CI**: A GitHub Actions workflow that leverages devenv for declarative and reproducible testing

## Note

Install `dioxus-cli`: `curl -fsSL https://dioxuslabs.com/install.sh | bash -s dx-v0.7.9`
