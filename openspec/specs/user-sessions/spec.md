# user-sessions Specification

## Purpose
The session cookie lifecycle (issue, lookup, expiry, sign-out) and the current-user endpoint the UI reads.

## Requirements

### Requirement: Session issuance
When sign-in completes the system SHALL mint a random session token of at least 122 bits, store only its SHA-256 in `sessions.token_hash` with the user id and an `expires_at` 30 days ahead, and set the cookie `session=<token>; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000`, adding `Secure` when the request arrived over HTTPS. If the sign-in request already carried a `session` cookie, the system SHALL delete that session's row, since the new cookie replaces it in the browser. The raw token SHALL never be persisted or logged.

#### Scenario: Signing in again replaces the session
- **WHEN** a browser with a valid `session` cookie completes sign-in
- **THEN** the previous session's row is deleted and only the new session remains valid

#### Scenario: Cookie attributes
- **WHEN** a sign-in completes over HTTPS
- **THEN** the `Set-Cookie` header is `session=<32 hex chars>; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000; Secure`

#### Scenario: Only the hash is stored
- **WHEN** the `sessions` table is inspected after sign-in
- **THEN** `token_hash` is the SHA-256 of the cookie value and no column holds the raw token

### Requirement: Session lookup
The system SHALL resolve the signed-in user from the `session` cookie by hashing it and selecting the session joined to its user where `expires_at` is in the future. A request without the cookie, with an unknown hash, or with an expired session SHALL be treated as anonymous without error. Requests without the cookie SHALL NOT query the database for a session.

#### Scenario: Valid session
- **WHEN** a request carries a `session` cookie whose hash matches an unexpired row
- **THEN** the request is attributed to that user

#### Scenario: Expired session
- **WHEN** a request carries a `session` cookie whose row has `expires_at` in the past
- **THEN** the request is anonymous

#### Scenario: No cookie
- **WHEN** a request carries no `session` cookie
- **THEN** the request is anonymous and no session query runs

### Requirement: Current user endpoint
The system SHALL expose a server function `GET /api/me` returning the signed-in user's display name and avatar URL, or nothing when anonymous.

#### Scenario: Signed in
- **WHEN** `/api/me` is called with a valid session
- **THEN** it returns `{ display_name, avatar_url }`

#### Scenario: Anonymous
- **WHEN** `/api/me` is called without a valid session
- **THEN** it returns no user and a success status

### Requirement: Navbar reflects sign-in state on first render
The navbar SHALL server-render a "Sign in" link to `/login?return_to=<current route>` when anonymous, and the user's avatar, display name, and a "Sign out" control when signed in. The user menu SHALL suspend independently so page content is not delayed by it.

#### Scenario: Anonymous visitor on a poll
- **WHEN** an anonymous browser loads `/p/abc123`
- **THEN** the initial HTML contains a "Sign in" link to `/login?return_to=/p/abc123`

#### Scenario: Signed-in visitor
- **WHEN** a browser with a valid session loads any page
- **THEN** the initial HTML shows the display name and a "Sign out" form posting to `/logout`

### Requirement: Sign out
`POST /logout` SHALL delete the session row matching the cookie, clear the cookie with `Max-Age=0` and the same `Path`, and respond 303 to `/`. The cookie SHALL be cleared even when no session row exists or the deletion fails. Sign-out SHALL NOT touch `vote_token` cookies; signed-in activity never creates them, so the session cookie is the only account state in the browser.

#### Scenario: Normal sign out
- **WHEN** a signed-in browser submits the "Sign out" form
- **THEN** its `sessions` row is gone, the response clears the `session` cookie, and the browser lands on `/` showing "Sign in"

#### Scenario: Sign out without a session
- **WHEN** `POST /logout` arrives without a `session` cookie
- **THEN** the response still clears the cookie and redirects to `/`

#### Scenario: Sign out leaves no voting trace
- **WHEN** a user votes on P while signed in, signs out, and reopens P
- **THEN** the browser holds no cookie from the signed-in session and the poll shows as not voted

### Requirement: Expired sessions are swept on a long interval
The server SHALL run a background task that deletes every session whose `expires_at` has passed, once at startup and then every 24 hours. Sign-in SHALL NOT delete sessions. A failed sweep SHALL be logged and SHALL NOT stop the task or affect request handling; the next interval retries.

#### Scenario: Sweep removes only expired sessions
- **WHEN** the sweep runs while `sessions` holds one expired and one unexpired row
- **THEN** only the unexpired row remains

#### Scenario: Sign-in leaves expired sessions to the sweep
- **WHEN** a user with an expired session signs in again
- **THEN** the expired row remains alongside the new one until the next sweep

#### Scenario: Sweep failure
- **WHEN** the sweep's query fails
- **THEN** the error is logged, the server keeps serving requests, and the sweep runs again at the next interval
