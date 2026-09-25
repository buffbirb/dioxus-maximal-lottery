## ADDED Requirements

### Requirement: Login page lists sign-in providers
The system SHALL serve a server-rendered page at `GET /login` that lists every registered identity provider as a button linking to `/login/{provider}` and carrying the sanitised `return_to` path. Provider buttons SHALL be plain links that perform a full navigation, not client-side route changes.

#### Scenario: Page shows a GitHub button
- **WHEN** a browser requests `/login`
- **THEN** the response contains a "Continue with GitHub" link whose target is `/login/github?return_to=/`

#### Scenario: Return path is carried through
- **WHEN** a browser requests `/login?return_to=/p/abc123`
- **THEN** the GitHub link target is `/login/github?return_to=/p/abc123`

### Requirement: Provider registry drives both the page and the routes
The system SHALL define identity providers in a single registry of `(id, label)` entries. The login page SHALL render one button per entry, and the server SHALL accept `/login/{id}` and `/login/{id}/callback` exactly for those ids. Adding a provider SHALL require no schema change.

#### Scenario: Unknown provider id
- **WHEN** a browser requests `/login/google` or `/login/google/callback` while only GitHub is registered
- **THEN** the server responds 404

#### Scenario: Registry and dispatch agree
- **WHEN** the unit test iterates the registry
- **THEN** every id resolves to a provider implementation whose id is the same string

### Requirement: Sign-in start redirects to the provider with CSRF state and PKCE
`GET /login/{provider}` SHALL generate a random `state` of at least 122 bits and a PKCE `code_verifier` of 64 lowercase hex characters, store both together with the sanitised `return_to` in a cookie `oauth_state` (value `<state>:<code_verifier>:<return_to>`) with `Path=/login`, `HttpOnly`, `SameSite=Lax`, `Max-Age=600`, and `Secure` when the request arrived over HTTPS, and SHALL redirect to the provider's authorization URL carrying `client_id`, `redirect_uri` equal to `{origin}/login/{provider}/callback`, `state`, `code_challenge` equal to the unpadded base64url SHA-256 of the verifier, and `code_challenge_method=S256`. The verifier itself SHALL never appear in a URL or a log. For GitHub the request SHALL carry no `scope`.

#### Scenario: PKCE challenge derivation
- **WHEN** the challenge is derived from the RFC 7636 Appendix B verifier
- **THEN** it equals the challenge given in that appendix

#### Scenario: Configured GitHub start
- **WHEN** GitHub credentials are configured and a browser requests `/login/github?return_to=/p/abc123`
- **THEN** the response is a redirect to `https://github.com/login/oauth/authorize` with `client_id`, `redirect_uri`, `state`, `code_challenge`, and `code_challenge_method=S256` query parameters and no `scope`, and it sets the `oauth_state` cookie whose verifier hashes to that `code_challenge`

#### Scenario: Secure flag follows the forwarded scheme
- **WHEN** the start request carries `X-Forwarded-Proto: https`
- **THEN** the `oauth_state` cookie includes `Secure`; without it the cookie omits `Secure`

### Requirement: Return path is restricted to same-origin app paths
The system SHALL reduce `return_to` to a safe path before use: missing or empty becomes `/`; any query or fragment is dropped; the value MUST start with `/` and MUST NOT start with `//` or `/\`; it MUST contain only `[A-Za-z0-9/._~-]`; it MUST NOT be `/login` or start with `/login/`; it MUST NOT exceed 256 bytes. Any value failing a rule becomes `/`.

#### Scenario: App paths are kept
- **WHEN** `return_to` is `/`, `/create`, `/p/abc123`, or `/p/abc123/results`
- **THEN** the same value is used

#### Scenario: Off-origin and unsafe values fall back
- **WHEN** `return_to` is `//evil.com`, `/\evil.com`, `https://evil.com`, `evil.com`, `/p/abc;x`, `/p/abc%2F`, or contains whitespace or non-ASCII bytes
- **THEN** `/` is used

#### Scenario: Login paths never loop
- **WHEN** `return_to` is `/login` or `/login/github`
- **THEN** `/` is used

### Requirement: Callback verifies state before doing anything else
`GET /login/{provider}/callback` SHALL read the `oauth_state` cookie, compare the presented `state` to the stored one in constant time, and clear the cookie on every outcome. A missing or malformed cookie (not exactly state, verifier, and return path), missing `code` or `state`, or a mismatch SHALL produce a 400 response and SHALL NOT contact the provider.

#### Scenario: Tampered or expired state
- **WHEN** a browser requests `/login/github/callback?code=x&state=y` without a matching `oauth_state` cookie
- **THEN** the server responds 400 and clears the `oauth_state` cookie

#### Scenario: User declined at the provider
- **WHEN** the callback arrives with an `error` query parameter such as `access_denied`
- **THEN** the server clears the `oauth_state` cookie and redirects to `/login`

### Requirement: Callback exchanges the code and normalises the profile
On a valid callback the system SHALL hand the provider the full callback (the `code`, the `code_verifier` from the `oauth_state` cookie, and every other callback parameter), exchange the code using the same `redirect_uri` as the start step and the verifier, fetch the profile, and normalise it to an identity of `(provider id, provider user id, optional display name, optional avatar URL)`. The provider user id is required; either profile field MAY be absent when the provider does not return it. For GitHub the provider user id SHALL be the numeric account id, the display name SHALL be the trimmed non-empty `name` or else the `login` (so it is always present), the avatar URL SHALL be `avatar_url` when present, and the access token SHALL be discarded after the profile fetch. Authorization codes, access tokens, and session tokens SHALL never be logged.

#### Scenario: Profile without a name
- **WHEN** GitHub returns `{ "id": 42, "login": "octocat", "name": "  " }`
- **THEN** the identity is `("github", "42", "octocat", none)`

#### Scenario: Token request carries the verifier
- **WHEN** the GitHub token exchange is built for a valid callback
- **THEN** its form body contains `code`, `redirect_uri`, `client_id`, `client_secret`, and the `code_verifier` stored in the `oauth_state` cookie

#### Scenario: Provider exchange fails
- **WHEN** GitHub answers the token or profile request with an error
- **THEN** the server responds 502, clears the `oauth_state` cookie, and logs the provider error without the code

#### Scenario: Provider not configured
- **WHEN** `OAUTH_GITHUB_CLIENT_ID` or `OAUTH_GITHUB_CLIENT_SECRET` is unset and a browser requests `/login/github`
- **THEN** the server responds 503 with a plain-text explanation, and the server logged one warning at startup

### Requirement: Sign-in creates or finds the account
The system SHALL look up `user_identities` by `(provider, provider_user_id)`. If absent it SHALL create a `users` row and the identity row in one transaction, using the identity's display name or, when absent, `"{provider label} user"`; if present it SHALL reuse the linked user and overwrite its display name and avatar only with values the identity carries, leaving a field unchanged when the identity omits it. Concurrent first sign-ins for the same identity SHALL result in exactly one user.

#### Scenario: First sign-in
- **WHEN** a GitHub account signs in for the first time
- **THEN** one `users` row and one `user_identities` row exist for it, and the session belongs to that user

#### Scenario: Returning sign-in with a renamed profile
- **WHEN** the same GitHub account signs in again with a new `name`
- **THEN** no new rows are created and `users.display_name` reflects the new name

#### Scenario: Returning sign-in without profile fields
- **WHEN** an existing user signs in through a provider whose identity carries no display name and no avatar
- **THEN** `users.display_name` and `users.avatar_url` keep their previous values

#### Scenario: First sign-in without a name
- **WHEN** a new identity without a display name signs in through a provider labelled "Apple"
- **THEN** the new user's `display_name` is `Apple user`

#### Scenario: Concurrent first sign-ins
- **WHEN** two callbacks for the same new identity commit at the same time
- **THEN** exactly one `users` row exists and both callbacks end signed in as that user

### Requirement: Successful sign-in establishes a session and returns to the app
After the account is resolved the system SHALL issue a session (see `user-sessions`), set its cookie together with the cleared `oauth_state` cookie, and respond 303 to the stored `return_to`.

#### Scenario: Round trip back to the poll
- **WHEN** a user starts sign-in from `/p/abc123` and authorizes at GitHub
- **THEN** the browser lands on `/p/abc123` with a `session` cookie set and no `oauth_state` cookie

### Requirement: Auth routes sit inside the app's middleware
The auth routes SHALL be registered on the axum router before the Basic Auth and tracing layers, so they are traced like every other request and protected by Basic Auth where that is enabled. Trace spans SHALL record the request path only, never the query string.

#### Scenario: Callback under Basic Auth
- **WHEN** the dev service, which requires Basic Auth, receives the redirect back from GitHub
- **THEN** the browser resends its cached credentials and the callback completes with the same single-use code
