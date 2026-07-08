CREATE TABLE polls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    share_id TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    description TEXT,
    deadline TEXT,
    hide_results INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE options (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    poll_id INTEGER NOT NULL REFERENCES polls (id) ON DELETE CASCADE,
    idx INTEGER NOT NULL,
    label TEXT NOT NULL
);

CREATE INDEX idx_options_poll_id ON options (poll_id);

CREATE TABLE votes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    poll_id INTEGER NOT NULL REFERENCES polls (id) ON DELETE CASCADE,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_votes_poll_id ON votes (poll_id);

CREATE TABLE vote_rankings (
    vote_id INTEGER NOT NULL REFERENCES votes (id) ON DELETE CASCADE,
    option_id INTEGER NOT NULL REFERENCES options (id) ON DELETE CASCADE,
    tier INTEGER NOT NULL
);

CREATE INDEX idx_vote_rankings_vote_id ON vote_rankings (vote_id);
