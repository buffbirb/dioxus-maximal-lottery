## Why

Maximal Lottery is passwordless and has no accounts. The only guard against repeat voting is the per-poll `vote_token` cookie, which clearing cookies defeats. Issue #44 asks for sign-in through external identity providers, starting with GitHub, built so Google and Apple can follow without schema changes. Issue #48, closed in favour of #44, adds "one account = one vote per poll", enforced by the database.

## What Changes

- A `/login` page listing providers, and routes `GET /login/{provider}`, `GET /login/{provider}/callback`, and `POST /logout` implementing the OAuth 2.0 authorization-code flow with CSRF `state`, PKCE (`S256`), and a same-origin `return_to`.
- A provider-agnostic account model: `users`, `user_identities` (unique on provider + subject; several per user allowed so linking needs no later migration, though this change creates one), and `sessions` (opaque token, hashed at rest, 30-day expiry). First sign-in creates the account; later sign-ins refresh only the profile fields the provider returned.
- `GET /api/me` returns the signed-in user's name and avatar; the navbar shows "Sign in" or the user with "Sign out".
- Signed-in votes record `user_id`, and a partial unique index on `(poll_id, user_id)` allows one vote per account per poll. A repeat vote behaves like the anonymous token case: an idempotent success that keeps the first ballot and sets the `voted` flag.
- This is one vote per account, not per person: an anonymous plus a signed-in vote, or sign-ins through two providers, still yield two votes.
- GitHub sign-in requests no scopes, uses the numeric GitHub id as subject, and never stores the access token.
- `OAUTH_GITHUB_CLIENT_ID` and `OAUTH_GITHUB_CLIENT_SECRET` configure GitHub. Without them the server logs a warning and `/login/github` answers 503, so local development and CI need no GitHub app.
- No behaviour change for anonymous voters, poll creation, or results.

## Capabilities

### New Capabilities
- `provider-login`: the `/login` page, OAuth routes, CSRF and PKCE, `return_to` handling, the provider abstraction, and account creation or lookup.
- `user-sessions`: the session cookie lifecycle (issue, lookup, expiry, sign-out) and the current-user endpoint.
- `account-vote-guard`: attributing votes to signed-in users and the one-account-one-vote rule.

### Modified Capabilities
<!-- openspec/specs/ is empty; no existing spec requirements change. -->

## Impact

- **Database**: one expand-only migration adding `users`, `user_identities`, `sessions`, `votes.user_id`, and `idx_votes_poll_id_user_id`. It must be applied locally before the sqlx macros compile.
- **api crate**: new `auth` module (providers, cookies, routes, `current_user`); vote queries take a user id; `origin.rs` moves here from the web crate.
- **web crate**: `/login` route and view, navbar user menu, auth router merged before the Basic Auth and trace layers.
- **Dependencies**: `reqwest`, `subtle`, and `base64` become optional server deps of api; all are already in `Cargo.lock`.
- **Deployment**: per environment, a GitHub OAuth App (callback `<origin>/login/github/callback`), two GitHub secrets, and matching `render.yaml` and `deploy.yml` entries. Deploys fail until the secrets exist, so create them before merging. Documented in `docs/deployment/README.md` and `docs/database/README.md`.
- **Out of scope**: account-only polls (#49), poll ownership (`polls.created_by`, needed for poll editing in #72), account linking UI, account deletion, session renewal.
