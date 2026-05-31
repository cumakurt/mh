CREATE TRIGGER IF NOT EXISTS commands_fts_ai AFTER INSERT ON commands BEGIN
  INSERT INTO commands_fts(rowid, command, cwd) VALUES (new.id, new.command, new.cwd);
END;

CREATE TRIGGER IF NOT EXISTS commands_fts_ad AFTER DELETE ON commands BEGIN
  INSERT INTO commands_fts(commands_fts, rowid, command, cwd)
  VALUES ('delete', old.id, old.command, old.cwd);
END;

CREATE TRIGGER IF NOT EXISTS commands_fts_au AFTER UPDATE ON commands BEGIN
  INSERT INTO commands_fts(commands_fts, rowid, command, cwd)
  VALUES ('delete', old.id, old.command, old.cwd);
  INSERT INTO commands_fts(rowid, command, cwd) VALUES (new.id, new.command, new.cwd);
END;
