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
TAPLO ?= taplo
ALEJANDRA ?= alejandra

.DEFAULT_GOAL := help

# -----------MAKEY------------
# Declare `<backend> <name> <version> [options]` in each row:
define MAKEY_PINS
cargo  dioxus-cli  0.7.9   --locked
cargo  taplo-cli   0.10.0  --locked
endef
# cargo  alejandra   3.1.0   --locked

include $(HOME)/.makey/common.mk

# Cross-compile targets, if any, go here:
# -----------------------------

# keep-sorted start block=yes

.PHONY: build
.PHONY: build-desktop
.PHONY: build-mobile
.PHONY: build-web
.PHONY: check
.PHONY: format
.PHONY: help
.PHONY: lint
.PHONY: pre-commit
.PHONY: serve
.PHONY: serve-desktop
.PHONY: serve-mobile
.PHONY: serve-web
.PHONY: test
.PHONY: up
.PHONY: update
build-desktop: ## Build the desktop client (release)
	$(DX) build --package desktop --platform desktop --release
build-mobile: ## Build the mobile client (release)
	$(DX) build --package mobile --platform mobile --release
build-web: ## Build the web client (release)
	$(DX) build --package web --platform web --release
build: build-web build-desktop ## Build the web and desktop clients (release)
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
serve-desktop: ## Run the desktop client with hot reload
	$(DX) serve --package desktop --platform desktop
serve-mobile: ## Run the mobile client with hot reload
	$(DX) serve --package mobile --platform mobile
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
