CREATE INDEX IF NOT EXISTS idx_commands_dedupe_lookup
  ON commands(command, cwd, started_at DESC);
