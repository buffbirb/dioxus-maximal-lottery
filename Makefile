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

.DEFAULT_GOAL := help

# keep-sorted start block=yes
.PHONY: build
build: build-web build-desktop ## Build the web and desktop clients (release)

.PHONY: build-desktop
build-desktop: ## Build the desktop client (release)
	$(DX) build --package desktop --platform desktop --release

.PHONY: build-mobile
build-mobile: ## Build the mobile client (release)
	$(DX) build --package mobile --platform mobile --release

.PHONY: build-web
build-web: ## Build the web client (release)
	$(DX) build --package web --platform web --release

.PHONY: check
check: ## Type-check the whole workspace, all targets and features
	$(CARGO) check --workspace --all-targets --all-features

.PHONY: clean
clean: ## Remove cargo and dx build artifacts
	$(CARGO) clean
	rm -rf target/dx

.PHONY: lint
lint: ## Lint the workspace with clippy; warnings are errors
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings

.PHONY: format
format: ## Format Rust source and rsx! macros in place
	$(CARGO) fmt --all
	$(DX) fmt

.PHONY: help
help: ## Show this help
	@grep -hE '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN {FS = ":.*?## "} {printf "\033[36m%-16s\033[0m %s\n", $$1, $$2}'

.PHONY: pre-commit
pre-commit: ## Run all pre-commit hooks against every file
	pre-commit run --all-files

.PHONY: serve
serve: serve-web ## Alias for serve-web

.PHONY: serve-desktop
serve-desktop: ## Run the desktop client with hot reload
	$(DX) serve --package desktop --platform desktop

.PHONY: serve-mobile
serve-mobile: ## Run the mobile client with hot reload
	$(DX) serve --package mobile --platform mobile

.PHONY: serve-web
serve-web: ## Run the web client with hot reload on $(WEB_HOST):$(WEB_PORT)
	$(DX) serve --package web --platform web --addr $(WEB_HOST) --port $(WEB_PORT)

.PHONY: test
test: ## Run the workspace test suite
	$(CARGO) test --workspace --all-features

.PHONY: up
up: ## Start the full devenv process stack (clickhouse, otel, web)
	devenv up

.PHONY: update
update: ## Update Cargo.lock to the latest compatible dependency versions
	$(CARGO) update
# keep-sorted end
