-- migrations/0001_init.sql
-- Run with: sqlx migrate run

CREATE TABLE IF NOT EXISTS users (
    id          TEXT PRIMARY KEY NOT NULL,       -- UUID v4
    username    TEXT UNIQUE NOT NULL,
    email       TEXT UNIQUE NOT NULL,
    password_hash TEXT NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    is_admin       BOOLEAN NOT NULL DEFAULT 0
);

-- Stores the set of ticker symbols a user has added
CREATE TABLE IF NOT EXISTS user_tickers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id     TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    ticker      TEXT NOT NULL,
    interval    TEXT NOT NULL DEFAULT '1d',
    range       TEXT NOT NULL DEFAULT '3mo',
    added_at    TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(user_id, ticker)
);

-- Index for fast per-user ticker lookups
CREATE INDEX IF NOT EXISTS idx_user_tickers_user_id ON user_tickers(user_id);

-- Trigger to auto-update updated_at on users
CREATE TRIGGER IF NOT EXISTS users_updated_at
AFTER UPDATE ON users
FOR EACH ROW
BEGIN
    UPDATE users SET updated_at = datetime('now') WHERE id = NEW.id;
END;

INSERT INTO users (id, username, email, password_hash, is_admin)
VALUES (
    'a1b2c3d4-e5f6-7a8b-9c0d-1e2f3a4b5c6d', 
    'admin', 
    'admin@example.com', 
    '$argon2id$v=19$m=19456,t=2,p=1$YK08OR+WSHi3jWMDvuVMgw$kbMTW94rkd7gkXj5nI05ZfvKM6nXk5ievX3IEAxfnS0', 
    1
) ON CONFLICT(username) DO NOTHING;