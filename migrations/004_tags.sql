CREATE TABLE IF NOT EXISTS tags (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    command_id  INTEGER NOT NULL REFERENCES commands(id) ON DELETE CASCADE,
    tag         TEXT NOT NULL,
    created_at  TEXT DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_tags_tag             ON tags(tag);
CREATE INDEX IF NOT EXISTS idx_tags_command_id      ON tags(command_id);
