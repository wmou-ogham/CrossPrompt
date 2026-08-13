ALTER TABLE vaults ADD COLUMN email TEXT;
ALTER TABLE vaults ADD COLUMN email_verified_at TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_vaults_email_unique
ON vaults(email)
WHERE email IS NOT NULL;

CREATE TABLE IF NOT EXISTS email_otp_challenges (
    id TEXT PRIMARY KEY NOT NULL,
    email TEXT NOT NULL,
    vault_id TEXT REFERENCES vaults(id) ON DELETE CASCADE,
    purpose TEXT NOT NULL CHECK (purpose IN ('login', 'bind')),
    code_digest BLOB NOT NULL,
    attempts_remaining INTEGER NOT NULL DEFAULT 5,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    consumed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_email_otp_email_purpose_created
ON email_otp_challenges(email, purpose, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_email_otp_expiry
ON email_otp_challenges(expires_at);

CREATE TABLE IF NOT EXISTS vault_email_sessions (
    token_digest BLOB PRIMARY KEY NOT NULL,
    vault_id TEXT NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_vault_email_sessions_vault
ON vault_email_sessions(vault_id);

CREATE INDEX IF NOT EXISTS idx_vault_email_sessions_expiry
ON vault_email_sessions(expires_at);

