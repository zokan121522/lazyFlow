-- Create users table
CREATE TABLE IF NOT EXISTS users (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    username        VARCHAR(64) UNIQUE NOT NULL,
    password_hash   TEXT NOT NULL,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    locked_until    TIMESTAMPTZ,
    failed_attempts INT DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
