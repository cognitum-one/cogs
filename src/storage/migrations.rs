//! Database migrations for PostgreSQL schema
//!
//! This module provides SQL migrations for creating and managing the database schema.
//! Migrations are applied using sqlx-cli: `sqlx migrate run`

// SQL migration constants - applied via sqlx-cli

/// SQL migration for creating the users table
pub const CREATE_USERS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY,
    username VARCHAR(255) UNIQUE NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    tier VARCHAR(50) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    is_active BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE INDEX IF NOT EXISTS idx_users_username ON users(username);
CREATE INDEX IF NOT EXISTS idx_users_tier ON users(tier);
CREATE INDEX IF NOT EXISTS idx_users_is_active ON users(is_active);
"#;

/// SQL migration for creating the api_keys table
pub const CREATE_API_KEYS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS api_keys (
    id VARCHAR(255) PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key_hash VARCHAR(255) NOT NULL,
    scope TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    revocation_reason TEXT,
    last_used_at TIMESTAMPTZ,
    usage_count BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_api_keys_user_id ON api_keys(user_id);
CREATE INDEX IF NOT EXISTS idx_api_keys_revoked ON api_keys(revoked_at);
CREATE INDEX IF NOT EXISTS idx_api_keys_expires ON api_keys(expires_at);
"#;

/// SQL migration for creating the refresh_tokens table
pub const CREATE_REFRESH_TOKENS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS refresh_tokens (
    id VARCHAR(255) PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    family_id VARCHAR(255) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    used_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_refresh_tokens_user_id ON refresh_tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_family_id ON refresh_tokens(family_id);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_expires ON refresh_tokens(expires_at);
CREATE INDEX IF NOT EXISTS idx_refresh_tokens_revoked ON refresh_tokens(revoked_at);
"#;

/// SQL migration for creating the audit_events table with vector support
pub const CREATE_AUDIT_EVENTS_TABLE: &str = r#"
-- Enable pgvector extension for vector embeddings
CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS audit_events (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users(id) ON DELETE SET NULL,
    event_type VARCHAR(100) NOT NULL,
    action VARCHAR(100) NOT NULL,
    resource VARCHAR(255) NOT NULL,
    status VARCHAR(50) NOT NULL,
    ip_address VARCHAR(45),
    user_agent TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    embedding vector(384),  -- 384-dimensional embeddings (e.g., all-MiniLM-L6-v2)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_audit_events_user_id ON audit_events(user_id);
CREATE INDEX IF NOT EXISTS idx_audit_events_event_type ON audit_events(event_type);
CREATE INDEX IF NOT EXISTS idx_audit_events_resource ON audit_events(resource);
CREATE INDEX IF NOT EXISTS idx_audit_events_status ON audit_events(status);
CREATE INDEX IF NOT EXISTS idx_audit_events_created_at ON audit_events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_events_metadata ON audit_events USING GIN(metadata);

-- Create HNSW index for fast vector similarity search
CREATE INDEX IF NOT EXISTS idx_audit_events_embedding ON audit_events
USING hnsw (embedding vector_cosine_ops)
WITH (m = 16, ef_construction = 64);
"#;

/// All migrations in order
pub const ALL_MIGRATIONS: &[&str] = &[
    CREATE_USERS_TABLE,
    CREATE_API_KEYS_TABLE,
    CREATE_REFRESH_TOKENS_TABLE,
    CREATE_AUDIT_EVENTS_TABLE,
];

/// Create migration files for sqlx-cli
pub fn generate_migration_files() -> Vec<(&'static str, &'static str)> {
    vec![
        ("001_create_users_table.sql", CREATE_USERS_TABLE),
        ("002_create_api_keys_table.sql", CREATE_API_KEYS_TABLE),
        ("003_create_refresh_tokens_table.sql", CREATE_REFRESH_TOKENS_TABLE),
        ("004_create_audit_events_table.sql", CREATE_AUDIT_EVENTS_TABLE),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrations_count() {
        assert_eq!(ALL_MIGRATIONS.len(), 4);
        assert_eq!(generate_migration_files().len(), 4);
    }

    #[test]
    fn test_migrations_not_empty() {
        for migration in ALL_MIGRATIONS {
            assert!(!migration.is_empty());
            assert!(migration.contains("CREATE TABLE"));
        }
    }
}
