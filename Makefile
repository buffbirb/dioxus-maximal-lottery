# Makefile for the Dioxus fullstack workspace.
#
# These targets wrap the cargo/dx commands you'd otherwise type by hand. They
# assume the devenv shell is active (run `devenv shell` or use direnv) so that
# `dx`, the wasm toolchain, and the pre-commit hooks are on PATH.

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

# -----------MAKEY------------
# 1. Toolchains `<language> <version> [component/workload]`:
define MAKEY_TOOLCHAINS
rust  stable  clippy,rustfmt
endef

# 2. Targets `<language> <targets>`:
define MAKEY_TARGETS
rust  wasm32-unknown-unknown
endef

# 3. Packages `<backend> <name> <version> [options]`:
define MAKEY_PACKAGES
cargo  dioxus-cli  0.7.9   --locked
cargo  taplo-cli   0.10.0  --locked
endef

# 4. Binaries `<host> <owner>/<repo> <version>`:
define MAKEY_BINARIES
github  theseus-rs/postgresql_binaries  18.4.0
github  F1bonacc1/process-compose       1.120.0
endef

# 5. Run `source .makey/activate`!

include $(HOME)/.makey/common.mk
# -----------------------------

# keep-sorted start block=yes

.PHONY: build
.PHONY: build-image
.PHONY: build-web
.PHONY: check
.PHONY: format
.PHONY: help
.PHONY: lint
.PHONY: pre-commit
.PHONY: serve
.PHONY: serve-all
.PHONY: serve-web
.PHONY: test
.PHONY: up
.PHONY: update
build-image: ## Bundle the web app and build the container image into ./result
	$(DX) bundle --package web --platform web --release
	$(NIX) build -f image.nix
build-web: ## Build the web client with dx (release)
	$(DX) build --package web --platform web --release
build: build-web ## Alias for build-web
check: ## Type-check the whole workspace, all targets and features
	$(CARGO) check --workspace --all-targets --all-features
format: ## Format Rust, rsx!, TOML, and Nix source in place
	$(CARGO) fmt --all
	$(DX) fmt
	git ls-files --cached --others --exclude-standard -z '*.toml' | xargs -0 $(TAPLO) fmt
	# git ls-files --cached --others --exclude-standard -z '*.nix' | xargs -0 $(ALEJANDRA)
help: ## Show this help
	@grep -hE '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN {FS = ":.*?## "} {printf "\033[36m%-16s\033[0m %s\n", $$1, $$2}'
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
	WEB_HOST=$(WEB_HOST) WEB_PORT=$(WEB_PORT) process-compose up -f process-compose.yaml --no-server
