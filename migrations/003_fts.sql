CREATE VIRTUAL TABLE IF NOT EXISTS commands_fts USING fts5(
    command,
    cwd,
    content='commands',
    content_rowid='id'
);
