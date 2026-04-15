//! PostgreSQL storage implementation with connection pooling
//!
//! Provides persistent storage for:
//! - User accounts and authentication
//! - API keys with scopes
//! - Refresh tokens with family tracking
//! - Audit events with vector embeddings

use sqlx::{PgPool, postgres::PgPoolOptions, Row};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use super::{StorageError, StorageResult};

/// PostgreSQL configuration
#[derive(Debug, Clone)]
pub struct PostgresConfig {
    /// Database connection string (postgres://user:pass@host:port/db)
    pub connection_string: String,
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Minimum number of connections in the pool
    pub min_connections: u32,
    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,
    /// Idle timeout in seconds
    pub idle_timeout_secs: u64,
}

impl Default for PostgresConfig {
    fn default() -> Self {
        Self {
            connection_string: "postgres://localhost/cognitum".to_string(),
            max_connections: 20,
            min_connections: 5,
            connection_timeout_secs: 30,
            idle_timeout_secs: 600,
        }
    }
}

/// PostgreSQL storage with connection pooling
pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    /// Create a new PostgreSQL store with the given configuration
    pub async fn new(config: PostgresConfig) -> StorageResult<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .acquire_timeout(std::time::Duration::from_secs(config.connection_timeout_secs))
            .idle_timeout(std::time::Duration::from_secs(config.idle_timeout_secs))
            .connect(&config.connection_string)
            .await
            .map_err(|e| StorageError::PoolError(e.to_string()))?;

        Ok(Self { pool })
    }

    /// Run database migrations
    pub async fn run_migrations(&self) -> StorageResult<()> {
        sqlx::migrate!("./migrations")
            .run(&self.pool)
            .await
            .map_err(|e| StorageError::Migration(e.to_string()))?;
        Ok(())
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Check database health
    pub async fn health_check(&self) -> StorageResult<bool> {
        sqlx::query("SELECT 1")
            .fetch_one(&self.pool)
            .await?;
        Ok(true)
    }
}

// ============================================================================
// User Storage
// ============================================================================

/// User record from database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserRecord {
    pub id: Uuid,
    pub username: String,
    pub password_hash: String,
    pub tier: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_active: bool,
}

impl PostgresStore {
    /// Create a new user
    pub async fn create_user(
        &self,
        username: &str,
        password_hash: &str,
        tier: &str,
    ) -> StorageResult<UserRecord> {
        let user = sqlx::query_as::<_, UserRecord>(
            r#"
            INSERT INTO users (id, username, password_hash, tier, created_at, updated_at, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, username, password_hash, tier, created_at, updated_at, is_active
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(username)
        .bind(password_hash)
        .bind(tier)
        .bind(Utc::now())
        .bind(Utc::now())
        .bind(true)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(db_err) = &e {
                if db_err.is_unique_violation() {
                    return StorageError::UserAlreadyExists(username.to_string());
                }
            }
            StorageError::Database(e)
        })?;

        Ok(user)
    }

    /// Get user by username
    pub async fn get_user_by_username(&self, username: &str) -> StorageResult<Option<UserRecord>> {
        let user = sqlx::query_as::<_, UserRecord>(
            "SELECT id, username, password_hash, tier, created_at, updated_at, is_active
             FROM users WHERE username = $1 AND is_active = true",
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    /// Get user by ID
    pub async fn get_user_by_id(&self, id: Uuid) -> StorageResult<Option<UserRecord>> {
        let user = sqlx::query_as::<_, UserRecord>(
            "SELECT id, username, password_hash, tier, created_at, updated_at, is_active
             FROM users WHERE id = $1 AND is_active = true",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(user)
    }

    /// Update user tier
    pub async fn update_user_tier(&self, id: Uuid, tier: &str) -> StorageResult<()> {
        let result = sqlx::query(
            "UPDATE users SET tier = $1, updated_at = $2 WHERE id = $3 AND is_active = true",
        )
        .bind(tier)
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::UserNotFound(id.to_string()));
        }

        Ok(())
    }

    /// Deactivate user (soft delete)
    pub async fn deactivate_user(&self, id: Uuid) -> StorageResult<()> {
        let result = sqlx::query(
            "UPDATE users SET is_active = false, updated_at = $1 WHERE id = $2",
        )
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::UserNotFound(id.to_string()));
        }

        Ok(())
    }

    /// Update user password hash
    pub async fn update_user_password(&self, id: Uuid, password_hash: &str) -> StorageResult<()> {
        let result = sqlx::query(
            "UPDATE users SET password_hash = $1, updated_at = $2 WHERE id = $3 AND is_active = true",
        )
        .bind(password_hash)
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::UserNotFound(id.to_string()));
        }

        Ok(())
    }
}

// ============================================================================
// API Key Storage
// ============================================================================

/// API key record from database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ApiKeyRecord {
    pub id: String,
    pub user_id: Uuid,
    pub key_hash: String,
    pub scope: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revocation_reason: Option<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub usage_count: i64,
}

impl PostgresStore {
    /// Store a new API key
    pub async fn store_api_key(
        &self,
        key_id: &str,
        user_id: Uuid,
        key_hash: &str,
        scope: &str,
        expires_at: Option<DateTime<Utc>>,
    ) -> StorageResult<()> {
        sqlx::query(
            r#"
            INSERT INTO api_keys (id, user_id, key_hash, scope, created_at, expires_at, usage_count)
            VALUES ($1, $2, $3, $4, $5, $6, 0)
            "#,
        )
        .bind(key_id)
        .bind(user_id)
        .bind(key_hash)
        .bind(scope)
        .bind(Utc::now())
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get API key by ID
    pub async fn get_api_key(&self, key_id: &str) -> StorageResult<Option<ApiKeyRecord>> {
        let key = sqlx::query_as::<_, ApiKeyRecord>(
            r#"
            SELECT id, user_id, key_hash, scope, created_at, expires_at,
                   revoked_at, revocation_reason, last_used_at, usage_count
            FROM api_keys
            WHERE id = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(key_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(key)
    }

    /// Revoke an API key
    pub async fn revoke_api_key(&self, key_id: &str, reason: &str) -> StorageResult<()> {
        let result = sqlx::query(
            "UPDATE api_keys SET revoked_at = $1, revocation_reason = $2 WHERE id = $3",
        )
        .bind(Utc::now())
        .bind(reason)
        .bind(key_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::ApiKeyNotFound(key_id.to_string()));
        }

        Ok(())
    }

    /// List all API keys for a user
    pub async fn list_user_api_keys(&self, user_id: Uuid) -> StorageResult<Vec<ApiKeyRecord>> {
        let keys = sqlx::query_as::<_, ApiKeyRecord>(
            r#"
            SELECT id, user_id, key_hash, scope, created_at, expires_at,
                   revoked_at, revocation_reason, last_used_at, usage_count
            FROM api_keys
            WHERE user_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(keys)
    }

    /// Update API key last used timestamp and increment usage count
    pub async fn update_api_key_usage(&self, key_id: &str) -> StorageResult<()> {
        sqlx::query(
            "UPDATE api_keys SET last_used_at = $1, usage_count = usage_count + 1 WHERE id = $2",
        )
        .bind(Utc::now())
        .bind(key_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Revoke all API keys for a user
    pub async fn revoke_user_api_keys(&self, user_id: Uuid, reason: &str) -> StorageResult<u64> {
        let result = sqlx::query(
            "UPDATE api_keys SET revoked_at = $1, revocation_reason = $2
             WHERE user_id = $3 AND revoked_at IS NULL",
        )
        .bind(Utc::now())
        .bind(reason)
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

// ============================================================================
// Refresh Token Storage
// ============================================================================

/// Refresh token record from database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RefreshTokenRecord {
    pub id: String,
    pub user_id: Uuid,
    pub family_id: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub used_at: Option<DateTime<Utc>>,
}

impl PostgresStore {
    /// Store a new refresh token
    pub async fn store_refresh_token(
        &self,
        token_id: &str,
        user_id: Uuid,
        family_id: &str,
        expires_at: DateTime<Utc>,
    ) -> StorageResult<()> {
        sqlx::query(
            r#"
            INSERT INTO refresh_tokens (id, user_id, family_id, created_at, expires_at)
            VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(token_id)
        .bind(user_id)
        .bind(family_id)
        .bind(Utc::now())
        .bind(expires_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get refresh token by ID
    pub async fn get_refresh_token(&self, token_id: &str) -> StorageResult<Option<RefreshTokenRecord>> {
        let token = sqlx::query_as::<_, RefreshTokenRecord>(
            r#"
            SELECT id, user_id, family_id, created_at, expires_at, revoked_at, used_at
            FROM refresh_tokens
            WHERE id = $1
            "#,
        )
        .bind(token_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(token)
    }

    /// Mark refresh token as used
    pub async fn mark_token_used(&self, token_id: &str) -> StorageResult<()> {
        let result = sqlx::query(
            "UPDATE refresh_tokens SET used_at = $1 WHERE id = $2",
        )
        .bind(Utc::now())
        .bind(token_id)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(StorageError::RefreshTokenNotFound(token_id.to_string()));
        }

        Ok(())
    }

    /// Revoke entire token family (for rotation detection)
    pub async fn revoke_token_family(&self, family_id: &str) -> StorageResult<u64> {
        let result = sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = $1 WHERE family_id = $2 AND revoked_at IS NULL",
        )
        .bind(Utc::now())
        .bind(family_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Revoke all refresh tokens for a user
    pub async fn revoke_user_tokens(&self, user_id: Uuid) -> StorageResult<u64> {
        let result = sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = $1 WHERE user_id = $2 AND revoked_at IS NULL",
        )
        .bind(Utc::now())
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }

    /// Clean up expired tokens
    pub async fn cleanup_expired_tokens(&self) -> StorageResult<u64> {
        let result = sqlx::query(
            "DELETE FROM refresh_tokens WHERE expires_at < $1",
        )
        .bind(Utc::now())
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

// ============================================================================
// Audit Log Storage with Vector Embeddings
// ============================================================================

/// Audit event record from database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEventRecord {
    pub id: Uuid,
    pub user_id: Option<Uuid>,
    pub event_type: String,
    pub action: String,
    pub resource: String,
    pub status: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

impl PostgresStore {
    /// Append an audit event with optional vector embedding
    pub async fn append_audit_event(
        &self,
        user_id: Option<Uuid>,
        event_type: &str,
        action: &str,
        resource: &str,
        status: &str,
        ip_address: Option<&str>,
        user_agent: Option<&str>,
        metadata: serde_json::Value,
        embedding: Option<&[f32]>,
    ) -> StorageResult<Uuid> {
        let event_id = Uuid::new_v4();

        if let Some(emb) = embedding {
            // Store with vector embedding for similarity search
            sqlx::query(
                r#"
                INSERT INTO audit_events
                (id, user_id, event_type, action, resource, status, ip_address, user_agent, metadata, embedding, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                "#,
            )
            .bind(event_id)
            .bind(user_id)
            .bind(event_type)
            .bind(action)
            .bind(resource)
            .bind(status)
            .bind(ip_address)
            .bind(user_agent)
            .bind(&metadata)
            .bind(emb)
            .bind(Utc::now())
            .execute(&self.pool)
            .await?;
        } else {
            // Store without embedding
            sqlx::query(
                r#"
                INSERT INTO audit_events
                (id, user_id, event_type, action, resource, status, ip_address, user_agent, metadata, created_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                "#,
            )
            .bind(event_id)
            .bind(user_id)
            .bind(event_type)
            .bind(action)
            .bind(resource)
            .bind(status)
            .bind(ip_address)
            .bind(user_agent)
            .bind(&metadata)
            .bind(Utc::now())
            .execute(&self.pool)
            .await?;
        }

        Ok(event_id)
    }

    /// Search audit events by vector similarity
    pub async fn search_audit_events(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> StorageResult<Vec<AuditEventRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, event_type, action, resource, status,
                   ip_address, user_agent, metadata, created_at
            FROM audit_events
            WHERE embedding IS NOT NULL
            ORDER BY embedding <-> $1
            LIMIT $2
            "#,
        )
        .bind(query_embedding)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let events = rows
            .into_iter()
            .map(|row| AuditEventRecord {
                id: row.get("id"),
                user_id: row.get("user_id"),
                event_type: row.get("event_type"),
                action: row.get("action"),
                resource: row.get("resource"),
                status: row.get("status"),
                ip_address: row.get("ip_address"),
                user_agent: row.get("user_agent"),
                metadata: row.get("metadata"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(events)
    }

    /// Get audit events for a user
    pub async fn get_user_audit_events(
        &self,
        user_id: Uuid,
        limit: usize,
        offset: usize,
    ) -> StorageResult<Vec<AuditEventRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, event_type, action, resource, status,
                   ip_address, user_agent, metadata, created_at
            FROM audit_events
            WHERE user_id = $1
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(user_id)
        .bind(limit as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await?;

        let events = rows
            .into_iter()
            .map(|row| AuditEventRecord {
                id: row.get("id"),
                user_id: row.get("user_id"),
                event_type: row.get("event_type"),
                action: row.get("action"),
                resource: row.get("resource"),
                status: row.get("status"),
                ip_address: row.get("ip_address"),
                user_agent: row.get("user_agent"),
                metadata: row.get("metadata"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(events)
    }

    /// Get audit events by resource
    pub async fn get_resource_audit_events(
        &self,
        resource: &str,
        limit: usize,
    ) -> StorageResult<Vec<AuditEventRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT id, user_id, event_type, action, resource, status,
                   ip_address, user_agent, metadata, created_at
            FROM audit_events
            WHERE resource = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(resource)
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;

        let events = rows
            .into_iter()
            .map(|row| AuditEventRecord {
                id: row.get("id"),
                user_id: row.get("user_id"),
                event_type: row.get("event_type"),
                action: row.get("action"),
                resource: row.get("resource"),
                status: row.get("status"),
                ip_address: row.get("ip_address"),
                user_agent: row.get("user_agent"),
                metadata: row.get("metadata"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(events)
    }

    /// Get audit event statistics
    pub async fn get_audit_statistics(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> StorageResult<serde_json::Value> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_events,
                COUNT(DISTINCT user_id) as unique_users,
                COUNT(CASE WHEN status = 'success' THEN 1 END) as successful_events,
                COUNT(CASE WHEN status = 'failure' THEN 1 END) as failed_events,
                json_agg(DISTINCT event_type) as event_types
            FROM audit_events
            WHERE created_at BETWEEN $1 AND $2
            "#,
        )
        .bind(from)
        .bind(to)
        .fetch_one(&self.pool)
        .await?;

        let stats = serde_json::json!({
            "total_events": row.get::<i64, _>("total_events"),
            "unique_users": row.get::<i64, _>("unique_users"),
            "successful_events": row.get::<i64, _>("successful_events"),
            "failed_events": row.get::<i64, _>("failed_events"),
            "event_types": row.get::<serde_json::Value, _>("event_types"),
        });

        Ok(stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_postgres_config_default() {
        let config = PostgresConfig::default();
        assert_eq!(config.max_connections, 20);
        assert_eq!(config.min_connections, 5);
    }
}
