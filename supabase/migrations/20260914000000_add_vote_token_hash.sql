ALTER TABLE votes ADD COLUMN token_hash BYTEA;

CREATE UNIQUE INDEX idx_votes_poll_id_token_hash
    ON votes (poll_id, token_hash)
    WHERE token_hash IS NOT NULL;
