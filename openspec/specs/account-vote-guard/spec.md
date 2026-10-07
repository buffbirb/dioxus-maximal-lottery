# account-vote-guard Specification

## Purpose
Attributing votes to signed-in users and enforcing one vote per account per poll, without changing the anonymous voter-token path.

## Requirements

### Requirement: Votes carry exactly one voter identity
A signed-in ballot SHALL record the user's id and a NULL `token_hash`. An anonymous ballot SHALL record the token hash and a NULL `user_id`, as today. The database SHALL enforce `CHECK (token_hash IS NULL OR user_id IS NULL)` on `votes`, so no row ties an account to a browser.

#### Scenario: Signed-in ballot
- **WHEN** a browser with a valid session submits a ballot, with or without a `vote_token` cookie
- **THEN** the `votes` row has `user_id` set to that user and `token_hash` NULL

#### Scenario: Anonymous ballot
- **WHEN** a browser without a valid session submits a ballot
- **THEN** the `votes` row has `user_id` NULL and `token_hash` set

### Requirement: Signed-in requests do not use voter tokens
For requests with a valid session, `create_poll`, `get_poll`, and `submit_vote` SHALL NOT issue a `vote_token` cookie and SHALL ignore any `vote_token` cookie the browser sends. A token cookie left by an earlier anonymous visit stays in the browser untouched. The browser still sends it on requests under that poll's path, because cookie scoping cannot depend on sign-in state, but the server never reads it on the signed-in path.

#### Scenario: First visit while signed in
- **WHEN** a signed-in browser with no `vote_token` cookie opens `/p/abc123`
- **THEN** the response has no `vote_token` `Set-Cookie` header

#### Scenario: Poll created while signed in
- **WHEN** a signed-in user creates a poll
- **THEN** the response has no `vote_token` `Set-Cookie` header

#### Scenario: Leftover anonymous token
- **WHEN** a browser that voted anonymously on P later signs in and opens P
- **THEN** the token cookie is neither read nor cleared, and the voted state comes from the account alone

### Requirement: One vote per account per poll
The database SHALL enforce a partial unique index on `votes (poll_id, user_id) WHERE user_id IS NOT NULL`. A repeat submission by the same account on the same poll, from any browser, SHALL return 200 without writing a row, keeping the first ballot. This matches how the anonymous token case responds, but the check uses only `user_id`.

#### Scenario: Same account, second browser
- **WHEN** a user who already voted on a poll submits another ballot from a different browser
- **THEN** the response is 200 and the poll's vote count is unchanged

#### Scenario: Retry after a lost response
- **WHEN** a signed-in browser resubmits the same ballot after a network failure
- **THEN** the response is 200 and exactly one vote exists for that account on that poll

#### Scenario: Concurrent submissions from one account
- **WHEN** two submissions from the same account reach the server at the same time
- **THEN** exactly one `votes` row is written and both requests succeed

#### Scenario: Two accounts in one browser
- **WHEN** account A votes on P, signs out, and account B signs in on the same browser and votes on P
- **THEN** B's vote is recorded, because nothing from A's vote is attached to the browser

### Requirement: Poll view reports voted state per identity
`get_poll` SHALL set `voted` from the signed-in user's votes alone when a valid session is present, and from the voter token alone otherwise, so the client renders the disabled "Already voted" button only for the identity that voted.

#### Scenario: Voted from another browser
- **WHEN** a signed-in user who voted on poll P from browser A opens P in browser B without ever voting there
- **THEN** the poll view has `voted = true` and the submit button is disabled

#### Scenario: Signed out after a signed-in vote
- **WHEN** a user votes on P while signed in, then signs out in the same browser and reloads P
- **THEN** the poll view has `voted = false`, so nothing on the page shows that the account voted

#### Scenario: Anonymous vote, then sign in
- **WHEN** a browser votes on P anonymously, then signs in to an account that has not voted on P, and reloads P
- **THEN** the poll view has `voted = false` and the account may vote

#### Scenario: Fresh voter
- **WHEN** a browser with a new token and no session, or a session whose user has not voted, opens P
- **THEN** the poll view has `voted = false`

### Requirement: Anonymous behaviour is unchanged
For requests without a valid session, the vote endpoints SHALL behave exactly as before this change: token-only deduplication, the same closed-poll and vote-cap checks in the same order, and the same responses.

#### Scenario: Anonymous duplicate
- **WHEN** an anonymous browser submits twice with the same `vote_token`
- **THEN** the second response is 200 and no second row is written

#### Scenario: Closed poll takes precedence
- **WHEN** any browser submits to a poll that is closed
- **THEN** the response is 400 "this poll is closed", whether or not the voter already voted

### Requirement: Vote insertion targets the caller's identity
The insert SHALL take exactly one identity. A signed-in insert SHALL pre-check and use `ON CONFLICT (poll_id, user_id) WHERE user_id IS NOT NULL DO NOTHING`. An anonymous insert SHALL keep the existing token pre-check and conflict target. Both SHALL keep the existing poll-row lock so cap accounting stays serialised.

#### Scenario: Account already voted
- **WHEN** a signed-in user who voted before submits again
- **THEN** no row is written and the response is 200
