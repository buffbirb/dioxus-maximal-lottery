-- Share ids are embedded in a `Set-Cookie` `Path` attribute built by string
-- concatenation, so the column must never hold a value that can break out of
-- that attribute. ASCII alphanumeric cannot express ';', ',', whitespace, or
-- control characters, and every id minted so far already satisfies this.
--
-- The drop makes the file safe to apply to a database that already carries the
-- constraint from an earlier migration layout.
ALTER TABLE polls
    DROP CONSTRAINT IF EXISTS polls_share_id_alphanumeric;
ALTER TABLE polls
    ADD CONSTRAINT polls_share_id_alphanumeric CHECK (share_id ~ '^[A-Za-z0-9]+$');
