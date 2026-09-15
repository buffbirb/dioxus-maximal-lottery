ALTER TABLE votes ADD COLUMN token_hash BYTEA;

CREATE UNIQUE INDEX idx_votes_poll_id_token_hash
    ON votes (poll_id, token_hash)
    WHERE token_hash IS NOT NULL;

-- Share ids are embedded in a `Set-Cookie` `Path` attribute built by string
-- concatenation, so the column must never hold a value that can break out of
-- that attribute. ASCII alphanumeric cannot express ';', ',', whitespace, or
-- control characters, and every id minted so far already satisfies this.
ALTER TABLE polls
    ADD CONSTRAINT polls_share_id_alphanumeric CHECK (share_id ~ '^[A-Za-z0-9]+$');
