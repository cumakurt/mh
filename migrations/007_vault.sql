CREATE TABLE IF NOT EXISTS vault (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    encrypted_data  BLOB NOT NULL,
    nonce           BLOB NOT NULL,
    label           TEXT,
    created_at      TEXT DEFAULT CURRENT_TIMESTAMP
);
