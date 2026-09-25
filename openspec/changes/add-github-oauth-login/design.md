## Context

- One Dioxus 0.7 fullstack binary (`packages/web`) serves SSR pages, the wasm bundle, and the `#[get]`/`#[post]` server functions declared in the `api` library crate. There is no separate API server; the client calls server functions as plain async fns over the same origin, so cookies flow automatically.
- The only identity mechanism today is the per-poll `vote_token` cookie (PR #70): random value, `Path=/p/{share_id}`, `HttpOnly`, SHA-256 stored in `votes.token_hash`, partial unique index on `(poll_id, token_hash)`, and an idempotent no-op on repeat submission. Cookies are parsed by hand in `api::cookies`; there is no cookie, session, or auth crate.
- Postgres is reached through sqlx with compile-time-checked macros against a live `DATABASE_URL`; there is no offline query cache, so migrations must be applied before code that uses new tables compiles. Supabase hosts the database only (`[auth] enabled = false`).
- Render terminates TLS; the process sees plain HTTP and learns the scheme and host from `X-Forwarded-*`, which `packages/web/src/origin.rs` already resolves (with a `PUBLIC_BASE_URL` override). The dev service sits behind HTTP Basic Auth.
- Migrations are expand-only and run before the image rolls out. Deploy syncs secrets to Render by name, and the `sync_var` helper fails the job when a value is empty.
- Constraints confirmed with the owner: two-table account model (`users` + `user_identities`), `/login` as a server-rendered chooser page, unit tests only.

## Goals / Non-Goals

**Goals:**
- GitHub sign-in that creates or finds an account and establishes a session, with correct CSRF and redirect handling.
- A provider seam where adding Google or Apple means one provider implementation and one registry entry, no schema change.
- One vote per account per poll, enforced by the database and surfaced exactly like the token guard (idempotent success plus the `voted` flag).
- Zero regression for anonymous voters; the server runs unchanged when the GitHub app is not configured.

**Non-Goals:**
- Polls that require an account (issue #49), poll ownership (needed for poll editing, issue #72), linking a second provider to an existing account from the UI, account deletion, session renewal or "remember me" choices, email collection, admin roles.

## Decisions

**Sessions are opaque server-side tokens, not signed cookies and not Supabase Auth.**
The session token is minted with the same generator as the vote token, only its SHA-256 is stored in `sessions.token_hash`, and the cookie is `session=…; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000` (`Secure` when the request came over HTTPS). Expiry is enforced in the lookup query and sign-out deletes the row. Alternatives: an HMAC-signed stateless cookie needs a server secret whose rotation logs everyone out and cannot revoke a single session; Supabase Auth/GoTrue would add a second identity system and JWT plumbing the app does not otherwise need. The chosen shape reuses the token-hash pattern the codebase already trusts and needs no secret.

**Account model: `users` + `user_identities` + `sessions`, and a nullable `votes.user_id`.**
`user_identities` is unique on `(provider, provider_user_id)`; `users` carries only what the UI shows (display name, avatar). A future provider adds rows, not columns. Profile fields are optional in the normalised identity because providers differ in what they return: Apple sends a name only on the first authorization and never an avatar, and Google may omit `name`. A first sign-in without a name stores the fallback `"{label} user"` (e.g. "Apple user"), and a returning sign-in refreshes only the fields the provider actually returned, so a later sign-in without profile data never erases what an earlier one stored. The GitHub subject is the numeric `id`, never the login, because logins can be renamed and reassigned. The schema allows many identities per user, but this change only ever produces one: sign-in either reuses the user an identity already points at or creates a new user with that single identity, and nothing attaches a second identity to an existing user. A later Google sign-in by the same person is therefore a separate account until linking exists. What the split buys is that linking needs no schema change: it inserts one more `user_identities` row for the session's user, and `sessions` and `votes` already reference `users.id`, not an identity. Alternatives: provider columns on `users` (a migration per provider), or `(provider, provider_user_id)` directly on `users`, which works until linking and then forces a table split and a repoint of every foreign key.

Linking is deferred until per-user state worth preserving exists (poll ownership, which poll editing in issue #72 needs). When it lands it should be explicit (a signed-in user starts `/login/{provider}` and the callback attaches the identity to the current user), never automatic by email (no email is collected, and email matching is an account-takeover vector), and it should refuse an identity that already belongs to another user rather than merge accounts, since a merge collides on `idx_votes_poll_id_user_id` whenever both accounts voted in the same poll.

**Provider dispatch is an enum, not a trait object.**
`Provider::{GitHub}` exposes `authorize_url(redirect_uri, state, code_challenge)` and `async fn exchange(&Callback, redirect_uri) -> Identity`. `Callback` carries the authorization code, the PKCE code verifier from the state cookie, and every other callback parameter, rather than the code alone, because the next providers need more than the code: Apple delivers the user's name in a `user` field on the callback itself, not in any token or profile response. GitHub ignores the extra parameters. `async fn` in traits is not dyn-compatible, the provider set is closed at compile time, and a `match` makes the compiler flag every missing arm when a variant is added. A shared, always-compiled `PROVIDERS` list of `(id, label)` drives both the login page buttons and the server registry; a unit test asserts they round-trip. Alternative: the `oauth2` crate. It would be a new dependency for two HTTP calls and a URL; GitHub's flow is simple enough that `reqwest` (already compiled through dioxus-fullstack) suffices.

**OAuth routes are axum handlers merged into the app before the layers; `/login` is a Dioxus page.**
`GET /login/{provider}`, `GET /login/{provider}/callback`, and `POST /logout` need redirects and several `Set-Cookie` headers. `FullstackContext::add_response_header` inserts (one header per name), and `get_poll` already spends that slot on `vote_token`, so server functions never set or clear the session cookie. Merging before the Basic Auth and trace layers keeps the routes traced and protected on dev; `dioxus::server::router` registers SSR as a fallback, so explicit routes win. The page's provider buttons are plain anchors (the client router would render NotFound for server-only paths); the navbar "Sign in" is a router `Link` because `/login` is a real client route.

**CSRF state, the PKCE verifier, and `return_to` travel in one short-lived cookie.**
`oauth_state=<state>:<code_verifier>:<return_to>; Path=/login; HttpOnly; SameSite=Lax; Max-Age=600`. Every sign-in uses PKCE with `S256` (RFC 7636), as the OAuth security BCP (RFC 9700) recommends even for confidential clients: the start step mints a verifier of 64 lowercase hex characters (two `cookies::new_token()` values), sends `code_challenge = BASE64URL-NOPAD(SHA-256(verifier))` with `code_challenge_method=S256`, and the callback passes the verifier to the token exchange. PKCE binds the code to the browser that started the flow, so an intercepted code is useless, and Google and Apple both support it; doing it now fixes the cookie format before a second provider depends on it. The three fields are split with `splitn(3, ':')`: hex never contains `:` and neither does a sanitised `return_to`. `SameSite=Lax` still sends it on the top-level GET back from GitHub, and `Path=/login` path-matches the callback. The callback compares `state` in constant time (`subtle`) and clears the cookie on every outcome. `return_to` is reduced to a strict path charset (`/`-rooted, not `//`, `[A-Za-z0-9/._~-]`, no `/login` prefix, length-capped, query and fragment dropped), so it can be embedded in an `href`, the cookie, and the `Location` header without an encoder and can never leave the origin. Alternative: a server-side state table; it adds writes and cleanup for no security gain here.

**Duplicate votes from an account mirror the token guard exactly.**
The issue comment "make duplicate votes fail like anonymous case" resolves to the current anonymous behaviour: an `EXISTS` pre-check inside the poll-locked transaction returns success without writing, `ON CONFLICT DO NOTHING` catches the race, and the poll's `voted` flag disables the button. The vote row stores both `token_hash` and `user_id`, so the pre-check is `token_hash = $2 OR user_id = $3` and the INSERT uses `ON CONFLICT DO NOTHING` with no target (a bare target covers every unique index, including both partial ones). Consequence: signing out in the same browser keeps "Already voted" via the token, and the same account in another browser is blocked via `user_id`.

**Configuration is optional at startup and named for the deploy pipeline.**
`OAUTH_GITHUB_CLIENT_ID` and `OAUTH_GITHUB_CLIENT_SECRET` are read once into a `OnceLock`; when absent the server logs one warning and `/login/github` answers 503, so devenv, CI, and `make go` need no GitHub app. The `OAUTH_` prefix is deliberate: GitHub Actions forbids secret names starting with `GITHUB_`, and `deploy.yml`'s `sync_var` requires the secret, workflow env, and Render key to share one name.

**The redirect URI is derived from the request, with `PUBLIC_BASE_URL` as the override.**
`origin.rs` moves from the web crate to `api::origin` unchanged (its only dependencies are `api::forwarded`, `http`, and `dioxus::cli_config`). The callback recomputes the same value, and GitHub validates it against the app's registered callback, so a spoofed `Host` cannot redirect the code elsewhere. Alternative: a dedicated `OAUTH_GITHUB_REDIRECT_URI` variable; it would drift from the share-link origin and add a per-environment value that the existing override already covers.

**GitHub specifics.** No scopes (public profile is enough); PKCE parameters are sent on authorize and token requests; the access token lives only inside `exchange` and is never stored or logged; `display_name` is the trimmed non-empty `name`, else `login`, so GitHub always supplies one; `avatar_url` is passed through when present; the client sends a `User-Agent` (GitHub rejects requests without one) and a 10 s timeout.

**Tests are unit tests over the pure parts** (authorize URL, `return_to` sanitiser, cookie headers, state parsing and matching, PKCE challenge derivation against the RFC 7636 Appendix B vector, profile field merging, profile mapping, registry round-trip, route parsing), built on hand-made `http::request::Parts` like `cookies.rs`. Database paths are verified manually against `devenv up`, matching how PR #70 shipped.

## Risks / Trade-offs

- [Two concurrent first sign-ins for the same GitHub account create two `users` rows] → the identity insert uses `ON CONFLICT DO NOTHING RETURNING user_id`; when it returns nothing the transaction rolls back (dropping the orphan user) and the lookup is retried once.
- [Deploys fail after this change until the two secrets exist in each GitHub environment, because `sync_var` exits on an empty value] → create the secrets and the GitHub OAuth Apps for dev and prd before merging; documented in the migration plan below.
- [Basic Auth on the dev service intercepts the redirect back from GitHub] → the 401 happens before the handler runs, the browser resends cached credentials and replays the same URL, so the single-use code is not consumed.
- [`make go` never applies the new migration to an existing makey database (`process-compose.yaml` only migrates when `public.polls` is missing, and sqlx-cli is not on that toolchain)] → documented reset or one-off `psql -f` of the migration file, which is safe to repeat.
- [Anonymous vote in one browser plus a signed-in vote in another yields two votes for one person] → inherent to a no-account default; issue #49 is the fix for creators who need it.
- [Signing out and signing in as a second GitHub account in the same browser is blocked from voting] → the token guard is the documented boundary; it protects against the far more common single-account repeat.
- [One person signing in with two providers gets two accounts and can vote twice per poll] → accepted until linking exists; linking would not stop deliberate abuse anyway, since anyone can open a second provider account.
- [`sessions` grows without a sweeper] → expired rows of a user are deleted on that user's next sign-in; a global sweep is a small follow-up.
- [`votes.user_id` has no `ON DELETE`, so deleting a user with votes fails] → intended; account deletion is out of scope and a future policy decision.
- [Origin derivation returns `None` behind an unexpected proxy chain] → `start` answers 500 and `PUBLIC_BASE_URL` pins the origin, as already documented for share links.
- [A provider that returns no profile fields leaves a stale name or avatar in place] → intended; keeping the last known value beats blanking it, and the user can refresh it by editing the profile at the provider that does send it.
- [`Cargo.lock` changes and CI runs `--locked`] → run `make check` after adding the deps and commit the lock; no new crates compile because reqwest 0.12 with `json` and `rustls-tls` is already present via dioxus-fullstack.
- [Cached wasm clients from before the deploy] → they never call the new endpoints and the vote endpoints keep their shapes, so nothing breaks.

## Migration Plan

1. Before merging: create one GitHub OAuth App per environment (callback `https://<host>/login/github/callback`), and add `OAUTH_GITHUB_CLIENT_ID` / `OAUTH_GITHUB_CLIENT_SECRET` to the `dev` and `prd` GitHub environments.
2. The migration `20260923000000_add_users_and_sessions.sql` is expand-only and re-runnable (`IF NOT EXISTS` throughout); the existing pipeline applies it before the rollout.
3. Rollback: redeploy the previous image with `run_migrations` unchecked. Old code ignores the new tables and the nullable `votes.user_id`.
4. Locally: `devenv up` applies the migration through `db:migrate`; export the two variables in the shell (or `devenv.local.nix`) only when the real sign-in flow is being exercised.

## Open Questions

- Session lifetime is set to 30 days with no renewal; adjust if the owner prefers browser-session cookies.
- Apple's callback, deferred to the change that adds Apple. Requesting the `name` scope forces `response_mode=form_post`, a cross-site POST that never carries the `SameSite=Lax` `oauth_state` cookie, so the GET-only callback cannot verify it. Options: (a) request no scopes, keep the GET callback, and accept the `"Apple user"` fallback name; (b) accept POST and have it 303 to the GET callback with the parameters in the query, so the top-level GET carries the Lax cookie unchanged, at the cost of the code and Apple's `user` JSON landing in browser history (the code is useless without the PKCE verifier); (c) a server-side state table the POST handler checks instead of the cookie. Per-provider `SameSite=None; Secure` also works but weakens the cookie and needs HTTPS locally. The choice depends on whether real names from Apple matter by then (poll ownership for poll editing, issue #72). All options add to the flow without changing the GitHub path or the cookie format.
- The navbar always offers "Sign in", even when the server has no GitHub configuration (clicking yields a 503 page). Hiding it would need the client to learn configuration state; deferred unless it proves confusing in local development.
