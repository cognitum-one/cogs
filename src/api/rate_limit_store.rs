//! Rate Limit Store - Trait and implementations for rate limit persistence
//!
//! Provides storage backends for rate limiting data with support for:
//! - In-memory storage for development/testing
//! - Redis storage for distributed production environments
//! - Automatic expiration and cleanup

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use parking_lot::RwLock;
use thiserror::Error;

#[cfg(test)]
use mockall::automock;

/// Errors that can occur during rate limit store operations
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Operation timeout")]
    Timeout,

    #[error("Invalid key format: {0}")]
    InvalidKey(String),

    #[error("Store unavailable: {0}")]
    Unavailable(String),
}

/// Trait for rate limit storage backends
///
/// This trait defines the interface for storing and retrieving rate limit
/// counters. Implementations can use different backends (Redis, in-memory, etc.)
#[cfg_attr(test, automock)]
pub trait RateLimitStore: Send + Sync {
    /// Increment the counter for a key within a time window
    ///
    /// # Arguments
    /// * `key` - Unique identifier (e.g., "api_key:sk_123:endpoint:/api/v1/simulate")
    /// * `window` - Duration of the rate limit window
    ///
    /// # Returns
    /// Current count after increment
    fn increment(&self, key: &str, window: Duration) -> Result<u64, StoreError>;

    /// Get current count for a key
    fn get_count(&self, key: &str) -> Result<u64, StoreError>;

    /// Reset counter for a key
    fn reset(&self, key: &str) -> Result<(), StoreError>;

    /// Get time until window reset
    fn get_reset_time(&self, key: &str) -> Result<SystemTime, StoreError>;
}

/// In-memory implementation of RateLimitStore
///
/// Uses a simple HashMap with expiring entries. Suitable for:
/// - Single-instance deployments
/// - Development and testing
/// - Low-traffic environments
///
/// NOT suitable for:
/// - Distributed systems (no shared state)
/// - High-traffic production (memory pressure)
pub struct InMemoryStore {
    data: Arc<RwLock<HashMap<String, RateLimitEntry>>>,
}

#[derive(Clone, Debug)]
struct RateLimitEntry {
    count: u64,
    window_start: SystemTime,
    window_duration: Duration,
}

impl InMemoryStore {
    /// Create a new in-memory rate limit store
    pub fn new() -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Clean up expired entries to prevent memory leak
    pub fn cleanup_expired(&self) {
        let now = SystemTime::now();
        let mut data = self.data.write();
        data.retain(|_, entry| {
            if let Ok(elapsed) = now.duration_since(entry.window_start) {
                elapsed < entry.window_duration
            } else {
                false
            }
        });
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimitStore for InMemoryStore {
    fn increment(&self, key: &str, window: Duration) -> Result<u64, StoreError> {
        let now = SystemTime::now();
        let mut data = self.data.write();

        let entry = data.entry(key.to_string()).or_insert_with(|| {
            RateLimitEntry {
                count: 0,
                window_start: now,
                window_duration: window,
            }
        });

        // Check if window has expired
        if let Ok(elapsed) = now.duration_since(entry.window_start) {
            if elapsed >= entry.window_duration {
                // Reset window
                entry.count = 0;
                entry.window_start = now;
                entry.window_duration = window;
            }
        }

        entry.count += 1;
        Ok(entry.count)
    }

    fn get_count(&self, key: &str) -> Result<u64, StoreError> {
        let now = SystemTime::now();
        let data = self.data.read();

        if let Some(entry) = data.get(key) {
            // Check if window has expired
            if let Ok(elapsed) = now.duration_since(entry.window_start) {
                if elapsed >= entry.window_duration {
                    return Ok(0); // Window expired
                }
            }
            Ok(entry.count)
        } else {
            Ok(0)
        }
    }

    fn reset(&self, key: &str) -> Result<(), StoreError> {
        let mut data = self.data.write();
        data.remove(key);
        Ok(())
    }

    fn get_reset_time(&self, key: &str) -> Result<SystemTime, StoreError> {
        let data = self.data.read();

        if let Some(entry) = data.get(key) {
            Ok(entry.window_start + entry.window_duration)
        } else {
            // No entry means window hasn't started, reset time is now
            Ok(SystemTime::now())
        }
    }
}

/// Redis implementation of RateLimitStore
///
/// Uses Redis with automatic expiration for distributed rate limiting.
/// Suitable for:
/// - Multi-instance production deployments
/// - High-traffic environments
/// - Distributed systems requiring shared state
///
/// # Redis Key Format
/// Keys follow the pattern: `ratelimit:{api_key}:{endpoint}`
///
/// # Expiration Strategy
/// Uses Redis TTL to automatically expire windows, preventing memory leaks
pub struct RedisStore {
    // In a real implementation, this would hold a Redis client
    // For now, we'll use a placeholder structure
    #[allow(dead_code)]
    connection_string: String,
}

impl RedisStore {
    /// Create a new Redis-backed rate limit store
    ///
    /// # Arguments
    /// * `connection_string` - Redis connection URL (e.g., "redis://localhost:6379")
    #[allow(dead_code)]
    pub fn new(connection_string: String) -> Self {
        Self { connection_string }
    }
}

// Placeholder implementation - in production this would use actual Redis commands
impl RateLimitStore for RedisStore {
    fn increment(&self, _key: &str, _window: Duration) -> Result<u64, StoreError> {
        // TODO: Implement with Redis INCR and EXPIRE commands
        Err(StoreError::Unavailable("Redis implementation pending".to_string()))
    }

    fn get_count(&self, _key: &str) -> Result<u64, StoreError> {
        // TODO: Implement with Redis GET command
        Err(StoreError::Unavailable("Redis implementation pending".to_string()))
    }

    fn reset(&self, _key: &str) -> Result<(), StoreError> {
        // TODO: Implement with Redis DEL command
        Err(StoreError::Unavailable("Redis implementation pending".to_string()))
    }

    fn get_reset_time(&self, _key: &str) -> Result<SystemTime, StoreError> {
        // TODO: Implement with Redis TTL command
        Err(StoreError::Unavailable("Redis implementation pending".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn in_memory_store_increments_counter() {
        let store = InMemoryStore::new();
        let key = "test_key";
        let window = Duration::from_secs(60);

        let count1 = store.increment(key, window).unwrap();
        let count2 = store.increment(key, window).unwrap();
        let count3 = store.increment(key, window).unwrap();

        assert_eq!(count1, 1);
        assert_eq!(count2, 2);
        assert_eq!(count3, 3);
    }

    #[test]
    fn in_memory_store_resets_after_window_expires() {
        let store = InMemoryStore::new();
        let key = "test_key";
        let window = Duration::from_millis(100);

        let count1 = store.increment(key, window).unwrap();
        assert_eq!(count1, 1);

        // Wait for window to expire
        thread::sleep(Duration::from_millis(150));

        let count2 = store.increment(key, window).unwrap();
        assert_eq!(count2, 1); // Reset to 1
    }

    #[test]
    fn in_memory_store_get_count_returns_current_count() {
        let store = InMemoryStore::new();
        let key = "test_key";
        let window = Duration::from_secs(60);

        store.increment(key, window).unwrap();
        store.increment(key, window).unwrap();

        let count = store.get_count(key).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn in_memory_store_reset_clears_counter() {
        let store = InMemoryStore::new();
        let key = "test_key";
        let window = Duration::from_secs(60);

        store.increment(key, window).unwrap();
        store.increment(key, window).unwrap();

        store.reset(key).unwrap();

        let count = store.get_count(key).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn in_memory_store_cleanup_removes_expired_entries() {
        let store = InMemoryStore::new();
        let window = Duration::from_millis(50);

        store.increment("key1", window).unwrap();
        store.increment("key2", window).unwrap();

        thread::sleep(Duration::from_millis(100));

        store.cleanup_expired();

        // All entries should be cleaned up
        let count1 = store.get_count("key1").unwrap();
        let count2 = store.get_count("key2").unwrap();

        assert_eq!(count1, 0);
        assert_eq!(count2, 0);
    }

    #[test]
    fn in_memory_store_get_reset_time_returns_future_time() {
        let store = InMemoryStore::new();
        let key = "test_key";
        let window = Duration::from_secs(60);

        store.increment(key, window).unwrap();

        let reset_time = store.get_reset_time(key).unwrap();
        let now = SystemTime::now();

        assert!(reset_time > now);
    }
}
