# Development

## Toolchain

Each tool is pinned in its own ecosystem's file, which CI, contributors and
Dependabot all read:

| File | Pins |
| --- | --- |
| `.pre-commit-config.yaml` | The git hooks and their revisions |
| `Cargo.lock` | Every crate and, through `dioxus` and `sqlx`, the `dx` and `sqlx-cli` versions |
| `rust-toolchain.toml` | The Rust version, `clippy`, `rustfmt` and the `wasm32-unknown-unknown` target |
| `tools/prek/package-lock.json` | [prek][prek], which runs the hooks |
| `tools/supabase/package-lock.json` | The Supabase CLI |

`dx` and `sqlx-cli` are released in lockstep with the `dioxus` and `sqlx`
crates, so they have no pin of their own: `tools/install-cargo-tools.sh`
installs the versions `Cargo.lock` resolves. A Dependabot bump to either crate
moves its CLI along with it.

`tools/setup.sh` installs what a checkout needs from these pins into one
directory, `.tools` unless you pass another: `dx`, `sqlx-cli`, prek and the
Supabase CLI, with their commands in its `bin/`, and the
[git hooks](#git-hooks). To set up by hand:

1. Install [rustup](https://rustup.rs/), [cargo-binstall][binstall] and
   [Node.js](https://nodejs.org/). The first `cargo` command in the repository
   installs the toolchain `rust-toolchain.toml` pins, or run
   `rustup toolchain install` to do it up front.
2. Run `tools/setup.sh`, and put `.tools/bin` on your PATH.

On macOS, the prebuilt `dx` links Homebrew's OpenSSL: `brew install openssl@3`.

### devenv

The devenv shell provides the toolchain, and installs prek and the git hooks
when you enter it.

### mise

`mise.toml` replaces steps 1 and 2. It installs Rust from `rust-toolchain.toml`,
Node.js and cargo-binstall, plus Postgres and process-compose for `make go`, and
its `setup` task runs `tools/setup.sh` into `.local/<os>-<arch>`, whose `bin/`
mise puts on your PATH. Once per clone:

```bash
mise trust
mise run setup
```

Then activate mise in your shell, with `mise activate` in your shell rc or
`eval "$(mise env)"` per session. Builds go under `.local/<os>-<arch>` too, so
the host and a Docker sandbox (`sbx`) that share the checkout keep them apart.

[binstall]: https://github.com/cargo-bins/cargo-binstall
[prek]: https://prek.j178.dev

## Git hooks

prek runs the hooks in `.pre-commit-config.yaml`; `tools/setup.sh` installs it
and the git hook. Run every hook on demand with `prek run --all-files`.

| Hook | Runs |
| --- | --- |
| `actionlint`, `keep-sorted` | Built from source with the Go on your PATH, or one prek downloads |
| `check-yaml`, `end-of-file-fixer`, `pretty-format-json`, `trailing-whitespace` | prek's built-in versions |
| `deadnix`, `nixfmt`, `statix` | Programs from your PATH, which the devenv shell provides |
| `dx fmt`, `rustfmt` | `dx` and `cargo fmt` from your PATH |
| `taplo-format` | taplo's binary from PyPI, in an environment prek installs |

The Nix hooks run only when a `.nix` file changes. Without their programs, skip
them: `SKIP=deadnix,nixfmt,statix git commit`. CI runs every hook but those
three.

## CI

`.github/workflows/ci.yml` runs on every pull request, whatever its base branch,
so each layer of a stacked pull request gets CI, and on pushes to `main`. Its
jobs run in parallel:

| Job | Does |
| --- | --- |
| Lint | Every hook but the Nix ones |
| Rust | Applies the migrations to a Postgres service, then runs clippy and the `api` tests |
| Web | Applies the migrations, then builds the fullstack app with `dx build` |

Rust and Web need a migrated database because sqlx checks queries against it at
compile time. On a pull request they run only when a file they build from
changes; `main` and manual runs always run everything.

The `CI` job is the required check. It needs every other job and fails if any
of them failed or was cancelled, so adding a job doesn't change the ruleset.
