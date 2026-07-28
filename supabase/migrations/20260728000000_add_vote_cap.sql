ALTER TABLE polls
    ADD COLUMN vote_cap INTEGER CHECK (vote_cap IS NULL OR vote_cap >= 1);
