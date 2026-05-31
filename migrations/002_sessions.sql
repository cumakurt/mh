CREATE TABLE IF NOT EXISTS sessions (
    id          TEXT PRIMARY KEY,
    username    TEXT,
    hostname    TEXT,
    shell       TEXT,
    tty         TEXT,
    is_ssh      INTEGER DEFAULT 0,
    started_at  TEXT,
    ended_at    TEXT
);
