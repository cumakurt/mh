CREATE TABLE IF NOT EXISTS commands (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    command         TEXT NOT NULL,
    command_hash    TEXT NOT NULL,
    cwd             TEXT,
    shell           TEXT,
    username        TEXT,
    hostname        TEXT,
    exit_code       INTEGER,
    duration_ms     INTEGER,
    started_at      TEXT NOT NULL,
    finished_at     TEXT,
    session_id      TEXT,
    tty             TEXT,
    is_ssh          INTEGER DEFAULT 0,
    is_root         INTEGER DEFAULT 0,
    git_repo        TEXT,
    git_branch      TEXT,
    git_commit      TEXT,
    category        TEXT,
    env_context     TEXT,
    is_pinned       INTEGER DEFAULT 0,
    is_masked       INTEGER DEFAULT 0,
    created_at      TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_commands_command     ON commands(command);
CREATE INDEX IF NOT EXISTS idx_commands_cwd         ON commands(cwd);
CREATE INDEX IF NOT EXISTS idx_commands_started_at  ON commands(started_at);
CREATE INDEX IF NOT EXISTS idx_commands_exit_code   ON commands(exit_code);
CREATE INDEX IF NOT EXISTS idx_commands_user        ON commands(username);
CREATE INDEX IF NOT EXISTS idx_commands_hostname    ON commands(hostname);
CREATE INDEX IF NOT EXISTS idx_commands_session     ON commands(session_id);
CREATE INDEX IF NOT EXISTS idx_commands_category    ON commands(category);
CREATE INDEX IF NOT EXISTS idx_commands_is_pinned   ON commands(is_pinned);
