CREATE INDEX IF NOT EXISTS idx_commands_command_hash ON commands(command_hash);
CREATE INDEX IF NOT EXISTS idx_commands_started_at_id ON commands(started_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_commands_purge_candidates
  ON commands(started_at ASC, id ASC)
  WHERE is_pinned = 0 AND is_legal_hold = 0;
CREATE INDEX IF NOT EXISTS idx_commands_git_repo ON commands(git_repo);
CREATE INDEX IF NOT EXISTS idx_commands_environment_tier ON commands(environment_tier);
CREATE UNIQUE INDEX IF NOT EXISTS idx_tags_command_tag ON tags(command_id, tag);
CREATE INDEX IF NOT EXISTS idx_audit_log_created_at ON audit_log(created_at DESC);

INSERT INTO commands_fts(commands_fts) VALUES('rebuild');
