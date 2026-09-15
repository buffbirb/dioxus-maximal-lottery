-- One opaque token per browser per poll, stored only as its SHA-256.
-- Legacy rows have no token and stay out of the index, so the column is
-- nullable and the index is partial.
ALTER TABLE votes ADD COLUMN token_hash BYTEA;

CREATE UNIQUE INDEX idx_votes_poll_id_token_hash
    ON votes (poll_id, token_hash)
    WHERE token_hash IS NOT NULL;
