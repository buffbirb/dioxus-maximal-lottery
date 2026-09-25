## ADDED Requirements

### Requirement: Votes are attributed to signed-in users
When a signed-in user submits a ballot, the stored vote SHALL record both the voter token hash and the user's id. Anonymous ballots SHALL continue to record only the token hash.

#### Scenario: Signed-in ballot
- **WHEN** a browser with a valid session and a `vote_token` cookie submits a ballot
- **THEN** the `votes` row has `user_id` set to that user and `token_hash` set to the token's hash

#### Scenario: Anonymous ballot
- **WHEN** a browser without a session submits a ballot
- **THEN** the `votes` row has `user_id` NULL and `token_hash` set

### Requirement: One vote per account per poll
The database SHALL enforce a partial unique index on `votes (poll_id, user_id) WHERE user_id IS NOT NULL`. A repeat submission by the same account on the same poll, from any browser, SHALL be handled exactly like the anonymous token case: the request succeeds with 200, no new row is written, and the first ballot is kept.

#### Scenario: Same account, second browser
- **WHEN** a user who already voted on a poll submits another ballot from a different browser
- **THEN** the response is 200 and the poll's vote count is unchanged

#### Scenario: Retry after a lost response
- **WHEN** a signed-in browser resubmits the same ballot after a network failure
- **THEN** the response is 200 and exactly one vote exists for that account on that poll

#### Scenario: Concurrent submissions from one account
- **WHEN** two submissions from the same account reach the server at the same time
- **THEN** exactly one `votes` row is written and both requests succeed

### Requirement: Poll view reports voted state for the account
`get_poll` SHALL set `voted` to true when either the request's voter token or the signed-in user already has a vote on the poll, so the client renders the disabled "Already voted" button.

#### Scenario: Voted from another browser
- **WHEN** a signed-in user who voted on poll P from browser A opens P in browser B without ever voting there
- **THEN** the poll view has `voted = true` and the submit button is disabled

#### Scenario: Signed out in the same browser
- **WHEN** a user votes on P while signed in, then signs out in the same browser and reloads P
- **THEN** the poll view has `voted = true` because the voter token already voted

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

### Requirement: Vote row insertion tolerates both identities
The insert SHALL use `ON CONFLICT DO NOTHING` without a conflict target so that a row carrying both a token hash and a user id is rejected silently by whichever unique index it violates, and the transaction SHALL keep the existing poll-row lock so cap accounting stays serialised.

#### Scenario: Token new, account already voted
- **WHEN** a signed-in user who voted before submits from a browser with a fresh token
- **THEN** no row is written and the response is 200
