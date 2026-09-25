## 1. Database migration

- [ ] 1.1 Add `supabase/migrations/20260923000000_add_users_and_sessions.sql` creating `users`, `user_identities` (`provider TEXT`, `provider_user_id TEXT` so Google and Apple subjects fit, unique on provider + provider_user_id, index on user_id), `sessions` (unique token_hash, index on user_id, expires_at), `votes.user_id` (nullable FK), and the partial unique index `idx_votes_poll_id_user_id`; every statement `IF NOT EXISTS`
- [ ] 1.2 Apply it locally (`devenv up` or `devenv tasks run db:migrate`) and confirm with `devenv tasks run db:info`

## 2. Dependencies and shared plumbing

- [ ] 2.1 Add `reqwest` (`default-features = false`, `json`, `rustls-tls`, version 0.12) to `[workspace.dependencies]`; add `reqwest`, `subtle`, and the existing workspace `base64` as optional deps of `packages/api` under the `server` feature; run `make check` and commit the updated `Cargo.lock`
- [ ] 2.2 Move `packages/web/src/origin.rs` to `packages/api/src/origin.rs` (drop the crate-level cfg, `crate::forwarded`, reword the module doc), export it from `api/src/lib.rs` under the server feature, delete the web `mod origin`, and point `components/share_section.rs` at `api::origin::derive_origin`; the 19 tests move unchanged
- [ ] 2.3 In `packages/api/src/cookies.rs` extract `pub fn value(parts, name)` from `token_from_request` and add a unit test reading a differently named cookie

## 3. Data access (`packages/api/src/db.rs`)

- [ ] 3.1 Add `UserRow { id, display_name, avatar_url }`
- [ ] 3.2 Add `upsert_identity(provider, provider_user_id, display_name: Option<&str>, avatar_url: Option<&str>, fallback_name: &str) -> i64`: lookup + profile refresh with `COALESCE(new, existing)` per field so absent values never overwrite, else insert user (display name or `fallback_name`) + identity with `ON CONFLICT (provider, provider_user_id) DO NOTHING RETURNING user_id`, rolling back and retrying once when a concurrent first sign-in wins
- [ ] 3.3 Add `insert_session` (prunes the user's expired rows first), `delete_session`, and `fetch_session_user` (join to users, `expires_at > NOW()`)
- [ ] 3.4 Change `has_voted(poll_id, token_hash: Option<&[u8]>, user_id: Option<i64>)` to `EXISTS (... token_hash = $2 OR user_id = $3)`, short-circuiting to false when both are None
- [ ] 3.5 Change `insert_vote` to take `user_id`, use the same combined pre-check, store both columns, and use `ON CONFLICT DO NOTHING` without a target; update the doc comment and the comment explaining the bare target

## 4. Auth module (`packages/api/src/auth/`)

- [ ] 4.1 `mod.rs`: `ProviderInfo`, `PROVIDERS` (github), always-compiled `return_to` module, server-gated submodules, `init()` that touches provider config so the unconfigured warning logs once
- [ ] 4.2 `return_to.rs`: `sanitize(Option<&str>) -> String` implementing the allowlist rules from the provider-login spec
- [ ] 4.3 `session.rs`: cookie name and 30-day max age, `token_from_request`, `set_header`, `clear_header`, `hash_from_context` (lock guard scoped to the fn), `async current_user_id()` that queries only when a cookie is present
- [ ] 4.4 `state.rs`: `oauth_state` cookie with `Path=/login` and 600 s, `new_verifier()` (two `cookies::new_token()` values, 64 hex chars), `challenge(verifier)` (unpadded base64url SHA-256), `set_header(state, verifier, return_to)`, `clear_header`, `parse` (`splitn(3, ':')`, rejecting missing fields or a verifier that is not 64 hex chars), constant-time `matches` via `subtle`
- [ ] 4.5 `provider.rs`: `Identity { provider, subject, display_name: Option<String>, avatar_url: Option<String> }`, `Callback { code, code_verifier, params }` (all other callback query parameters), `AuthError { Unconfigured, Upstream }`, `enum Provider { GitHub }` with `from_id`, `id`, `label`, `authorize_url(redirect_uri, state, code_challenge)`, `async exchange(&Callback, redirect_uri)`
- [ ] 4.6 `github.rs`: env config in a `OnceLock<Option<Config>>` (`OAUTH_GITHUB_CLIENT_ID`, `OAUTH_GITHUB_CLIENT_SECRET`), shared `reqwest::Client` with user agent and timeout, `authorize_url` via `Url::parse_with_params` with `code_challenge` and `code_challenge_method=S256` and without `scope`, `exchange` (form POST to the token endpoint including `code_verifier`, with `Accept: application/json`, then `GET /user` with bearer, `application/vnd.github+json`, and API version headers), pure `identity_from_profile`
- [ ] 4.7 `routes.rs`: axum `router()` with `GET /login/{provider}` (404/503/500 paths, state cookie, redirect), `GET /login/{provider}/callback` (state check, `error` handling, build `Callback` from the query and the cookie's verifier, exchange, upsert with the provider label's fallback name, session, two `Set-Cookie` via `AppendHeaders`, 303 to `return_to`), `POST /logout` (delete row if present, clear cookie, 303 `/`); never log code, state, or tokens
- [ ] 4.8 `#[get("/api/me")] current_user() -> Result<Option<UserView>, ServerFnError>` in `auth/mod.rs`, and `UserView { display_name, avatar_url }` in `model.rs`

## 5. Vote attribution (`packages/api/src/polls.rs`)

- [ ] 5.1 In `get_poll`, resolve `user_id` with `auth::session::current_user_id()` after the sync token read and pass it to `has_voted`
- [ ] 5.2 In `submit_vote`, resolve `user_id` the same way and pass it to `insert_vote`; leave `create_poll` and the closed/cap ordering unchanged

## 6. Web (`packages/web`)

- [ ] 6.1 Add `#[route("/login?:return_to")] Login { return_to: Option<String> }` before the catch-all in `main.rs`; call `api::auth::init()` after `init_pool`; merge `api::auth::routes::router()` into the app before the Basic Auth and trace layers
- [ ] 6.2 Add `views/login.rs` (register in `views/mod.rs`): sanitised `return_to`, heading, one-line note, one plain `a.cta-button.login-provider` per `PROVIDERS` entry with an inline GitHub SVG mark and "Continue with {label}"
- [ ] 6.3 Add `UserMenu` to `components/navbar.rs` inside its own `SuspenseBoundary`: `use_server_future(api::auth::current_user)`; signed out renders a `Link` to `Route::Login { return_to: current route }`; signed in renders avatar, display name, and a native `form method=post action=/logout` with a "Sign out" button
- [ ] 6.4 CSS: `.navbar-right` gap, `.user-menu`, `.user-avatar`, `.user-name`, `.signout-button` in `navbar.css`; `#login` in the page-frame list plus `.login-providers` and `.login-provider` in `main.css`
- [ ] 6.5 Update `docs/web/spec.md` with the `/login` route and the navbar user menu

## 7. Deployment and docs

- [ ] 7.1 Add `OAUTH_GITHUB_CLIENT_ID` and `OAUTH_GITHUB_CLIENT_SECRET` (`sync: false`) to the prd and dev services in `render.yaml`
- [ ] 7.2 Map both secrets in the `env:` block of the "Sync Render environment variables" step in `.github/workflows/deploy.yml` and add two `sync_var` lines
- [ ] 7.3 Document the two secrets and the per-environment GitHub OAuth App (callback `<origin>/login/github/callback`, local `http://127.0.0.1:8080/login/github/callback`, 503 when unset) in `docs/deployment/README.md`
- [ ] 7.4 Document local setup (export the vars or use `devenv.local.nix`) and the `make go` migration gap (reset the makey postgres dir or `psql -f` the migration) in `docs/database/README.md`
- [ ] 7.5 Create the GitHub OAuth Apps and the GitHub environment secrets for dev and prd before merging, since `sync_var` fails deploys on empty values

## 8. Unit tests

- [ ] 8.1 `auth::return_to`: fallback to `/`, app paths kept, query and fragment dropped, off-origin and unsafe bytes rejected, `/login` paths rejected, overlong rejected
- [ ] 8.2 `auth::session` and `auth::state`: header attributes (path, flags, max-age, `Secure` only when requested), cookie reading among others, state parse and malformed cases (two fields, empty verifier, non-hex verifier), `matches` rejects length and content mismatches, `challenge` matches the RFC 7636 Appendix B vector, verifiers are 64 hex chars
- [ ] 8.3 `auth::provider` and `auth::github`: registry round-trips through `from_id`/`id`, unknown ids rejected, authorize URL carries exactly `client_id`, `redirect_uri`, `state`, `code_challenge`, `code_challenge_method` and no `scope`, token request form carries `code_verifier`, display name falls back to login, avatar is `None` when absent, subject is the numeric id
- [ ] 8.4 `web::main` route tests: `/login` with and without `return_to`; `/login/github`, `/login/github/callback`, `/logout` remain page misses on the client

## 9. Verification

- [ ] 9.1 `make check`, `make lint`, `make test`, then `make format` and confirm the diff contains only intended formatting
- [ ] 9.2 Manual end-to-end against `devenv up` with a local GitHub OAuth App: sign in from a poll and return to it, navbar shows the user, vote then reload shows "Already voted", same account in a second browser shows "Already voted", sign out keeps the token guard, anonymous flow unchanged, duplicate cURL submits return 200 with no new row, tampered callback returns 400, declined consent returns to `/login`
- [ ] 9.3 Confirm startup without the two env vars logs one warning and `/login/github` returns 503; confirm trace spans carry the path only
