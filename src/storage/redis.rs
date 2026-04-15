//! Redis-backed storage for rate limiting and session management
//!
//! Provides distributed storage with:
//! - Fixed window rate limiting
//! - Sliding window rate limiting (more accurate)
//! - Session caching with TTL
//! - Atomic operations
//! - Automatic key expiration

use redis::{Client, AsyncCommands, aio::ConnectionManager, RedisError};
use std::time::{Duration, SystemTime};
use crate::api::rate_limit_store::{RateLimitStore, StoreError};
use super::StorageError;

impl From<RedisError> for StorageError {
    fn from(err: RedisError) -> Self {
        StorageError::Redis(err.to_string())
    }
}

/// Redis configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Redis connection URL (e.g., "redis://localhost:6379")
    pub url: String,

    /// Connection pool size
    pub pool_size: usize,

    /// Connection timeout in seconds
    pub connect_timeout_seconds: u64,

    /// Command timeout in seconds
    pub command_timeout_seconds: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://127.0.0.1:6379".to_string(),
            pool_size: 10,
            connect_timeout_seconds: 5,
            command_timeout_seconds: 3,
        }
    }
}

impl RedisConfig {
    /// Create a new Redis configuration
    pub fn new(url: String) -> Self {
        Self {
            url,
            ..Default::default()
        }
    }

    /// Set pool size
    pub fn with_pool_size(mut self, size: usize) -> Self {
        self.pool_size = size;
        self
    }

    /// Set connection timeout
    pub fn with_connect_timeout(mut self, seconds: u64) -> Self {
        self.connect_timeout_seconds = seconds;
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), StorageError> {
        if self.url.is_empty() {
            return Err(StorageError::InvalidConfiguration(
                "Redis URL cannot be empty".to_string(),
            ));
        }

        if self.pool_size == 0 {
            return Err(StorageError::InvalidConfiguration(
                "Pool size must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }
}

/// Redis store for distributed storage
pub struct RedisStore {
    conn: ConnectionManager,
}

impl RedisStore {
    /// Create a new Redis store
    ///
    /// # Arguments
    /// * `config` - Redis configuration
    ///
    /// # Errors
    /// Returns error if connection fails or configuration is invalid
    pub async fn new(config: RedisConfig) -> Result<Self, StorageError> {
        config.validate()?;

        let client = Client::open(config.url.as_str())
            .map_err(|e| StorageError::Redis(format!("Failed to create Redis client: {}", e)))?;

        let conn = ConnectionManager::new(client)
            .await
            .map_err(|e| StorageError::Redis(format!("Failed to connect to Redis: {}", e)))?;

        Ok(Self { conn })
    }

    /// Check if the connection is healthy
    pub async fn ping(&self) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await?;
        Ok(())
    }

    // ========================================================================
    // Rate Limiting - Fixed Window
    // ========================================================================

    /// Fixed window rate limiting
    ///
    /// Uses atomic increment with expiry. Simple and efficient but can allow
    /// bursts at window boundaries.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for rate limit bucket
    /// * `limit` - Maximum requests allowed in window
    /// * `window_seconds` - Window duration in seconds
    ///
    /// # Returns
    /// RateLimitResult with allowed status and metadata
    pub async fn rate_limit_check(
        &self,
        key: &str,
        limit: u64,
        window_seconds: u64,
    ) -> Result<RateLimitResult, StorageError> {
        let now = chrono::Utc::now().timestamp() as u64;
        let window_key = format!("rate:{}:{}", key, now / window_seconds);

        let mut conn = self.conn.clone();

        // Atomic increment with expiry
        let mut pipe = redis::pipe();
        pipe.atomic()
            .incr(&window_key, 1)
            .expire(&window_key, (window_seconds + 1) as i64)
            .ignore();

        let (count,): (u64,) = pipe.query_async(&mut conn).await?;

        let remaining = if count <= limit { limit - count } else { 0 };
        let reset_at = ((now / window_seconds) + 1) * window_seconds;

        Ok(RateLimitResult {
            allowed: count <= limit,
            remaining,
            reset_at,
            limit,
        })
    }

    // ========================================================================
    // Rate Limiting - Sliding Window
    // ========================================================================

    /// Sliding window rate limiting (more accurate)
    ///
    /// Uses Redis sorted sets to track individual requests. More accurate than
    /// fixed window but slightly more expensive.
    ///
    /// # Arguments
    /// * `key` - Unique identifier for rate limit bucket
    /// * `limit` - Maximum requests allowed in window
    /// * `window_seconds` - Window duration in seconds
    ///
    /// # Returns
    /// RateLimitResult with allowed status and metadata
    pub async fn sliding_window_check(
        &self,
        key: &str,
        limit: u64,
        window_seconds: u64,
    ) -> Result<RateLimitResult, StorageError> {
        let now = chrono::Utc::now().timestamp_millis() as f64;
        let window_ms = (window_seconds * 1000) as f64;
        let window_start = now - window_ms;

        let sorted_set_key = format!("rate:sliding:{}", key);
        let mut conn = self.conn.clone();

        // Remove old entries, add new, count
        let mut pipe = redis::pipe();
        pipe.atomic()
            .zrembyscore(&sorted_set_key, 0, window_start as i64)
            .ignore()
            .zadd(&sorted_set_key, now.to_string(), now as i64)
            .ignore()
            .zcard(&sorted_set_key)
            .expire(&sorted_set_key, (window_seconds + 1) as i64)
            .ignore();

        let results: Vec<u64> = pipe.query_async(&mut conn).await?;
        let count = results.first().copied().unwrap_or(0);

        let remaining = if count <= limit { limit - count } else { 0 };
        let reset_at = (now as u64 / 1000) + window_seconds;

        Ok(RateLimitResult {
            allowed: count <= limit,
            remaining,
            reset_at,
            limit,
        })
    }

    // ========================================================================
    // Session Cache
    // ========================================================================

    /// Cache a session token
    ///
    /// # Arguments
    /// * `token_hash` - Hashed token identifier
    /// * `user_id` - User ID associated with token
    /// * `ttl_seconds` - Time to live in seconds
    pub async fn cache_session(
        &self,
        token_hash: &str,
        user_id: &str,
        ttl_seconds: u64,
    ) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        let key = format!("session:{}", token_hash);

        conn.set_ex::<_, _, ()>(&key, user_id, ttl_seconds)
            .await
            .map_err(|e| StorageError::Redis(format!("Failed to cache session: {}", e)))?;

        Ok(())
    }

    /// Get cached session
    ///
    /// # Arguments
    /// * `token_hash` - Hashed token identifier
    ///
    /// # Returns
    /// Some(user_id) if session exists and valid, None otherwise
    pub async fn get_cached_session(&self, token_hash: &str) -> Result<Option<String>, StorageError> {
        let mut conn = self.conn.clone();
        let key = format!("session:{}", token_hash);

        let result: Option<String> = conn.get(&key)
            .await
            .map_err(|e| StorageError::Redis(format!("Failed to get cached session: {}", e)))?;

        Ok(result)
    }

    /// Invalidate a specific session
    ///
    /// # Arguments
    /// * `token_hash` - Hashed token identifier
    pub async fn invalidate_session(&self, token_hash: &str) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        let key = format!("session:{}", token_hash);

        let _: u64 = conn.del(&key)
            .await
            .map_err(|e| StorageError::Redis(format!("Failed to invalidate session: {}", e)))?;

        Ok(())
    }

    /// Invalidate all sessions for a user
    ///
    /// # Arguments
    /// * `user_id` - User ID to invalidate sessions for
    ///
    /// # Returns
    /// Number of sessions invalidated
    pub async fn invalidate_user_sessions(&self, user_id: &str) -> Result<u64, StorageError> {
        let mut conn = self.conn.clone();
        let pattern = "session:*";

        // Scan for all session keys
        let mut cursor = 0;
        let mut total_deleted = 0u64;

        loop {
            let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await?;

            // Filter keys that match this user
            for key in keys {
                if let Ok(Some(stored_user_id)) = conn.get::<_, Option<String>>(&key).await {
                    if stored_user_id == user_id {
                        let _: u64 = conn.del(&key).await?;
                        total_deleted += 1;
                    }
                }
            }

            cursor = new_cursor;
            if cursor == 0 {
                break;
            }
        }

        Ok(total_deleted)
    }

    // ========================================================================
    // Generic Key-Value Operations
    // ========================================================================

    /// Set a key with expiry
    pub async fn set_with_ttl(&self, key: &str, value: &str, ttl_seconds: u64) -> Result<(), StorageError> {
        let mut conn = self.conn.clone();
        conn.set_ex::<_, _, ()>(key, value, ttl_seconds).await?;
        Ok(())
    }

    /// Get a value by key
    pub async fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        let mut conn = self.conn.clone();
        let result: Option<String> = conn.get(key).await?;
        Ok(result)
    }

    /// Delete a key
    pub async fn delete(&self, key: &str) -> Result<bool, StorageError> {
        let mut conn = self.conn.clone();
        let deleted: u64 = conn.del(key).await?;
        Ok(deleted > 0)
    }

    /// Check if key exists
    pub async fn exists(&self, key: &str) -> Result<bool, StorageError> {
        let mut conn = self.conn.clone();
        let exists: bool = conn.exists(key).await?;
        Ok(exists)
    }

    /// Get TTL for a key
    pub async fn ttl(&self, key: &str) -> Result<i64, StorageError> {
        let mut conn = self.conn.clone();
        let ttl: i64 = conn.ttl(key).await?;
        Ok(ttl)
    }
}

/// Rate limit result
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,

    /// Remaining requests in window
    pub remaining: u64,

    /// Unix timestamp when the window resets
    pub reset_at: u64,

    /// Maximum requests allowed
    pub limit: u64,
}

// ========================================================================
// RateLimitStore Trait Implementation
// ========================================================================

impl RateLimitStore for RedisStore {
    fn increment(&self, key: &str, window: Duration) -> Result<u64, StoreError> {
        // Use tokio runtime for async in sync context
        let rt = tokio::runtime::Handle::try_current()
            .map_err(|_| StoreError::Unavailable("No tokio runtime available".to_string()))?;

        rt.block_on(async {
            let result = self.rate_limit_check(key, u64::MAX, window.as_secs())
                .await
                .map_err(|e| StoreError::ConnectionFailed(e.to_string()))?;

            Ok(result.limit - result.remaining)
        })
    }

    fn get_count(&self, key: &str) -> Result<u64, StoreError> {
        let rt = tokio::runtime::Handle::try_current()
            .map_err(|_| StoreError::Unavailable("No tokio runtime available".to_string()))?;

        rt.block_on(async {
            let mut conn = self.conn.clone();
            let count: u64 = conn.get(key).await.unwrap_or(0);
            Ok(count)
        })
    }

    fn reset(&self, key: &str) -> Result<(), StoreError> {
        let rt = tokio::runtime::Handle::try_current()
            .map_err(|_| StoreError::Unavailable("No tokio runtime available".to_string()))?;

        rt.block_on(async {
            let mut conn = self.conn.clone();
            let _: u64 = conn.del(key)
                .await
                .map_err(|e| StoreError::ConnectionFailed(e.to_string()))?;
            Ok(())
        })
    }

    fn get_reset_time(&self, key: &str) -> Result<SystemTime, StoreError> {
        let rt = tokio::runtime::Handle::try_current()
            .map_err(|_| StoreError::Unavailable("No tokio runtime available".to_string()))?;

        rt.block_on(async {
            let mut conn = self.conn.clone();
            let ttl: i64 = conn.ttl(key).await.unwrap_or(60);
            Ok(SystemTime::now() + Duration::from_secs(ttl.max(0) as u64))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create test config
    fn test_config() -> RedisConfig {
        RedisConfig {
            url: std::env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            pool_size: 5,
            connect_timeout_seconds: 2,
            command_timeout_seconds: 1,
        }
    }

    // Helper to check if Redis is available
    async fn redis_available() -> bool {
        match RedisStore::new(test_config()).await {
            Ok(store) => store.ping().await.is_ok(),
            Err(_) => false,
        }
    }

    #[tokio::test]
    async fn test_redis_config_validation() {
        let config = RedisConfig::default();
        assert!(config.validate().is_ok());

        let invalid_config = RedisConfig {
            url: "".to_string(),
            ..Default::default()
        };
        assert!(invalid_config.validate().is_err());

        let invalid_pool = RedisConfig {
            pool_size: 0,
            ..Default::default()
        };
        assert!(invalid_pool.validate().is_err());
    }

    #[tokio::test]
    async fn test_redis_connection() {
        if !redis_available().await {
            eprintln!("Redis not available, skipping test");
            return;
        }

        let config = test_config();
        let store = RedisStore::new(config).await.unwrap();
        assert!(store.ping().await.is_ok());
    }

    #[tokio::test]
    async fn test_fixed_window_rate_limiting() {
        if !redis_available().await {
            eprintln!("Redis not available, skipping test");
            return;
        }

        let store = RedisStore::new(test_config()).await.unwrap();
        let key = "test:fixed:rate_limit";
        let limit = 5u64;
        let window = 2u64;

        // First 5 requests should be allowed
        for i in 1..=5 {
            let result = store.rate_limit_check(key, limit, window).await.unwrap();
            assert!(result.allowed, "Request {} should be allowed", i);
            assert_eq!(result.limit, limit);
            assert_eq!(result.remaining, limit - i);
        }

        // 6th request should be denied
        let result = store.rate_limit_check(key, limit, window).await.unwrap();
        assert!(!result.allowed);
        assert_eq!(result.remaining, 0);

        // Clean up
        let _ = store.delete(key).await;
    }

    #[tokio::test]
    async fn test_session_cache() {
        if !redis_available().await {
            eprintln!("Redis not available, skipping test");
            return;
        }

        let store = RedisStore::new(test_config()).await.unwrap();
        let token_hash = "test_token_hash_123";
        let user_id = "user_456";
        let ttl = 60u64;

        // Cache session
        store.cache_session(token_hash, user_id, ttl).await.unwrap();

        // Retrieve session
        let cached = store.get_cached_session(token_hash).await.unwrap();
        assert_eq!(cached, Some(user_id.to_string()));

        // Invalidate session
        store.invalidate_session(token_hash).await.unwrap();

        // Should be None now
        let cached = store.get_cached_session(token_hash).await.unwrap();
        assert_eq!(cached, None);
    }
}
