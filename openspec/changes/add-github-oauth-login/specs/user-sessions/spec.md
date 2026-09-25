## ADDED Requirements

### Requirement: Session issuance
When sign-in completes the system SHALL mint a random session token of at least 122 bits, store only its SHA-256 in `sessions.token_hash` with the user id and an `expires_at` 30 days ahead, and set the cookie `session=<token>; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000`, adding `Secure` when the request arrived over HTTPS. The raw token SHALL never be persisted or logged.

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
`POST /logout` SHALL delete the session row matching the cookie, clear the cookie with `Max-Age=0` and the same `Path`, and respond 303 to `/`. The cookie SHALL be cleared even when no session row exists or the deletion fails.

#### Scenario: Normal sign out
- **WHEN** a signed-in browser submits the "Sign out" form
- **THEN** its `sessions` row is gone, the response clears the `session` cookie, and the browser lands on `/` showing "Sign in"

#### Scenario: Sign out without a session
- **WHEN** `POST /logout` arrives without a `session` cookie
- **THEN** the response still clears the cookie and redirects to `/`

### Requirement: Expired sessions are pruned per user
When a user signs in, the system SHALL delete that user's sessions whose `expires_at` has passed before inserting the new one.

#### Scenario: Old sessions removed on sign-in
- **WHEN** a user with two expired sessions signs in again
- **THEN** only the new session row remains for that user
