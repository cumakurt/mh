CREATE TABLE IF NOT EXISTS audit_log (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type  TEXT NOT NULL,
    raw_command TEXT,
    reason      TEXT,
    username    TEXT,
    hostname    TEXT,
    created_at  TEXT DEFAULT CURRENT_TIMESTAMP
);
