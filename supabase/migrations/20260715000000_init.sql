CREATE TABLE polls (
    id BIGSERIAL PRIMARY KEY,
    share_id TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    description TEXT,
    deadline TIMESTAMPTZ,
    hide_results BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_polls_share_id ON polls (share_id);

CREATE TABLE options (
    id BIGSERIAL PRIMARY KEY,
    poll_id BIGINT NOT NULL REFERENCES polls (id) ON DELETE CASCADE,
    idx INTEGER NOT NULL,
    label TEXT NOT NULL,
    CONSTRAINT label_length CHECK (char_length(label) <= 200)
);

CREATE INDEX idx_options_poll_id ON options (poll_id);

CREATE TABLE votes (
    id BIGSERIAL PRIMARY KEY,
    poll_id BIGINT NOT NULL REFERENCES polls (id) ON DELETE CASCADE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_votes_poll_id ON votes (poll_id);

CREATE TABLE vote_rankings (
    vote_id BIGINT NOT NULL REFERENCES votes (id) ON DELETE CASCADE,
    option_id BIGINT NOT NULL REFERENCES options (id) ON DELETE CASCADE,
    tier INTEGER NOT NULL,
    PRIMARY KEY (vote_id, option_id)
);

CREATE INDEX idx_vote_rankings_vote_id ON vote_rankings (vote_id);
