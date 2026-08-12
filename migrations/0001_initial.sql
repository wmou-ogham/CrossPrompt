PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS vaults (
    id TEXT PRIMARY KEY NOT NULL,
    secret_hash BLOB NOT NULL UNIQUE,
    name TEXT NOT NULL DEFAULT 'My CrossPrompt',
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'suspended', 'deleted')),
    ever_used INTEGER NOT NULL DEFAULT 0,
    suspended_reason TEXT,
    deleted_by TEXT CHECK (deleted_by IN ('user', 'admin') OR deleted_by IS NULL),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT
);

CREATE TABLE IF NOT EXISTS blocks (
    id TEXT PRIMARY KEY NOT NULL,
    vault_id TEXT NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    position INTEGER NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_blocks_vault_position ON blocks(vault_id, position);

CREATE TABLE IF NOT EXISTS bundles (
    id TEXT PRIMARY KEY NOT NULL,
    vault_id TEXT NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    block_ids TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_bundles_vault ON bundles(vault_id);

CREATE TABLE IF NOT EXISTS revisions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vault_id TEXT NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    action TEXT NOT NULL,
    before_json TEXT,
    after_json TEXT,
    source TEXT NOT NULL DEFAULT 'web',
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_revisions_vault_created ON revisions(vault_id, id DESC);

CREATE TABLE IF NOT EXISTS notification_targets (
    vault_id TEXT PRIMARY KEY NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('pushcut', 'ntfy', 'generic_json')),
    encrypted_config BLOB NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS admin_sessions (
    token_digest BLOB PRIMARY KEY NOT NULL,
    csrf_digest BLOB NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_admin_sessions_expiry ON admin_sessions(expires_at);

CREATE TABLE IF NOT EXISTS admin_audit_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    action TEXT NOT NULL,
    vault_id TEXT,
    reason TEXT,
    ip_hash TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_admin_audit_created ON admin_audit_logs(id DESC);

CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    vault_id TEXT NOT NULL REFERENCES vaults(id) ON DELETE CASCADE,
    target_type TEXT NOT NULL,
    status_code INTEGER,
    success INTEGER NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_created ON webhook_deliveries(created_at DESC);

CREATE TABLE IF NOT EXISTS creation_limits (
    ip_hash TEXT NOT NULL,
    bucket TEXT NOT NULL,
    count INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (ip_hash, bucket)
);

