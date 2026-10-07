-- Accounts for provider sign-in. A user can hold several identities so that
-- linking a second provider later needs no migration; sessions and votes
-- reference the user, never an identity.
CREATE TABLE IF NOT EXISTS users (
    id BIGSERIAL PRIMARY KEY,
    display_name TEXT NOT NULL,
    avatar_url TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- provider_user_id is TEXT: GitHub's subject is numeric, Google's and
-- Apple's are opaque strings.
CREATE TABLE IF NOT EXISTS user_identities (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    provider_user_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (provider, provider_user_id)
);

CREATE INDEX IF NOT EXISTS idx_user_identities_user_id ON user_identities (user_id);

-- Only SHA-256 of the session cookie is stored.
CREATE TABLE IF NOT EXISTS sessions (
    id BIGSERIAL PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions (user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions (expires_at);

-- No ON DELETE: deleting a user who voted is a policy decision not made yet.
ALTER TABLE votes ADD COLUMN IF NOT EXISTS user_id BIGINT REFERENCES users (id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_votes_poll_id_user_id
    ON votes (poll_id, user_id)
    WHERE user_id IS NOT NULL;

-- A vote belongs to an account or a browser, never both: tying the two
-- would keep the account's vote visible in the browser after sign-out.
-- ADD CONSTRAINT has no IF NOT EXISTS, hence the guard.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'votes_one_voter_identity'
          AND conrelid = 'votes'::regclass
    ) THEN
        ALTER TABLE votes
            ADD CONSTRAINT votes_one_voter_identity
            CHECK (token_hash IS NULL OR user_id IS NULL);
    END IF;
END
$$;
