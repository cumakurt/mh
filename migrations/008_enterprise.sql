ALTER TABLE audit_log ADD COLUMN prev_hash TEXT NOT NULL DEFAULT '';
ALTER TABLE audit_log ADD COLUMN entry_hash TEXT NOT NULL DEFAULT '';

ALTER TABLE commands ADD COLUMN is_legal_hold INTEGER NOT NULL DEFAULT 0;
ALTER TABLE commands ADD COLUMN environment_tier TEXT;

CREATE TABLE IF NOT EXISTS legal_holds (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    label       TEXT NOT NULL,
    session_id  TEXT,
    command_id  INTEGER,
    tag         TEXT,
    git_repo    TEXT,
    created_at  TEXT DEFAULT CURRENT_TIMESTAMP,
    reason      TEXT
);

CREATE INDEX IF NOT EXISTS idx_legal_holds_session ON legal_holds(session_id);
CREATE INDEX IF NOT EXISTS idx_legal_holds_command ON legal_holds(command_id);

CREATE TABLE IF NOT EXISTS runbooks (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    name              TEXT NOT NULL UNIQUE,
    description       TEXT,
    source_session_id TEXT,
    created_at        TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at        TEXT
);

CREATE TABLE IF NOT EXISTS runbook_steps (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    runbook_id  INTEGER NOT NULL REFERENCES runbooks(id) ON DELETE CASCADE,
    step_order  INTEGER NOT NULL,
    command     TEXT NOT NULL,
    cwd         TEXT,
    notes       TEXT
);

CREATE INDEX IF NOT EXISTS idx_runbook_steps_runbook ON runbook_steps(runbook_id);

CREATE TABLE IF NOT EXISTS purge_audit (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    action      TEXT NOT NULL,
    target      TEXT,
    count       INTEGER NOT NULL DEFAULT 0,
    username    TEXT,
    hostname    TEXT,
    created_at  TEXT DEFAULT CURRENT_TIMESTAMP
);
