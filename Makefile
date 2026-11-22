# Makefile for the Dioxus fullstack workspace.
#
# These targets wrap the cargo/dx commands you'd otherwise type by hand. Most
# use whichever toolchain shell is active, so run them from `devenv shell` (or
# direnv) or a shell with mise activated; from neither, `dx`, the wasm toolchain
# and DATABASE_URL are missing. Targets that only make sense under devenv (`up`,
# `pre-commit`) need the devenv shell.

# Web dev server address (mirrors processConfigs.web in devenv.nix).
WEB_HOST ?= 127.0.0.1
WEB_PORT ?= 8080

# Client platform crates and the feature that activates each one.
CARGO ?= cargo
DX ?= dx
NIX ?= nix
TAPLO ?= taplo
ALEJANDRA ?= alejandra

.DEFAULT_GOAL := help

# Toolchain: versions and the `setup` task in mise.toml. On the host mise uses
# its global dirs: `mise trust` once, then `make install`; activate a shell with
# `mise activate` in your rc or `eval "$(mise env -s zsh)"`. sbx runs
# `mise run setup` inside the container on its own. Only the recipes that need
# mise's tools specifically (`go`, `db-*`, `clean`) evaluate `mise env`
# themselves; the rest use whatever shell they're run from.
SHELL := /bin/sh
MISE_ENV := eval "$$(mise env -s bash)"

# keep-sorted start block=yes

.PHONY: build
.PHONY: build-image
.PHONY: build-web
.PHONY: check
.PHONY: clean
.PHONY: db-info
.PHONY: db-migrate
.PHONY: db-reset
.PHONY: distclean
.PHONY: format
.PHONY: help
.PHONY: install
.PHONY: lint
.PHONY: pre-commit
.PHONY: serve
.PHONY: serve-all
.PHONY: serve-web
.PHONY: test
.PHONY: up
.PHONY: update
# dx bundles into cargo's target dir, which mise.toml moves under .local/ when
# mise is active; image.nix's default only covers a plain ./target.
build-image: ## Bundle the web app and build the container image into ./result
	$(DX) bundle --package web --platform web --release
	$(NIX) build -f image.nix --arg artifact "$${CARGO_TARGET_DIR:-$(CURDIR)/target}/dx/web/release/web"
build-web: ## Build the web client with dx (release)
	$(DX) build --package web --platform web --release
build: build-web ## Alias for build-web
check: ## Type-check the whole workspace, all targets and features
	$(CARGO) check --workspace --all-targets --all-features
# PGDATA is <root>/state/postgres. The mise/cargo/rustup dirs under <root> only
# exist in the sbx container's tree; the host's are mise's global dirs.
clean: ## Remove this platform's in-repo tools and caches (keeps the postgres data dir)
	@$(MISE_ENV); root="$$(dirname "$$(dirname "$$PGDATA")")"; \
	rm -rf "$$root/mise" "$$root/mise-cache" "$$root/mise-state" "$$root/cargo" "$$root/rustup" "$$root/bin" "$$root/target"
# The db-* targets mirror devenv's db:* tasks against the `make go` Postgres.
db-info: ## Show migration status of the local Postgres
	@$(MISE_ENV); sqlx migrate info --source supabase/migrations
db-migrate: ## Apply pending migrations to the local Postgres
	@$(MISE_ENV); sqlx migrate run --source supabase/migrations
db-reset: ## Drop and recreate the local public schema (re-run db-migrate after)
	@$(MISE_ENV); psql "$$DATABASE_URL" -v ON_ERROR_STOP=1 -c 'DROP SCHEMA public CASCADE; CREATE SCHEMA public;'
distclean: ## Remove every platform's toolchain tree, including postgres data
	rm -rf "$(CURDIR)/.local"
format: ## Format Rust, rsx!, TOML, and Nix source in place
	$(CARGO) fmt --all
	$(DX) fmt
	git ls-files --cached --others --exclude-standard -z '*.toml' | xargs -0 $(TAPLO) fmt
	# git ls-files --cached --others --exclude-standard -z '*.nix' | xargs -0 $(ALEJANDRA)
help: ## Show this help
	@grep -hE '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN {FS = ":.*?## "} {printf "\033[36m%-16s\033[0m %s\n", $$1, $$2}'
# Same as `mise run setup`; sbx runs the task itself.
install: ## Install the toolchain pinned in mise.toml
	@mise run setup || { echo "make: mise run setup failed — did you 'mise trust'?" >&2; exit 1; }
lint: ## Lint the workspace with clippy; warnings are errors
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings
pre-commit: ## Run all pre-commit hooks against every file
	prek run --all-files
serve-web: ## Run the web client with hot reload on $(WEB_HOST):$(WEB_PORT)
	$(DX) serve --package web --platform web --addr $(WEB_HOST) --port $(WEB_PORT)
serve: serve-web ## Alias for serve-web
test: ## Run the workspace test suite
	$(CARGO) test --workspace --all-features
up: ## Start the full devenv process stack (clickhouse, otel, web)
	devenv up
update: ## Update Cargo.lock to the latest compatible dependency versions
	$(CARGO) update
# keep-sorted end

.PHONY: go
go: ## Run postgres + migrations + the web client together via process-compose
	$(MISE_ENV); WEB_HOST=$(WEB_HOST) WEB_PORT=$(WEB_PORT) process-compose up -f process-compose.yaml --no-server
