//! Integration tests for Redis-backed rate limiting
//!
//! These tests require a running Redis instance.
//! Set REDIS_URL environment variable to override default connection.

use cognitum::storage::{RedisStore, RedisConfig, RateLimitResult};
use cognitum::api::rate_limit_store::RateLimitStore;
use std::time::Duration;

// Helper to check if Redis is available
async fn redis_available() -> bool {
    let config = RedisConfig::default();
    match RedisStore::new(config).await {
        Ok(store) => store.ping().await.is_ok(),
        Err(_) => false,
    }
}

#[tokio::test]
async fn test_redis_fixed_window_rate_limiting() {
    if !redis_available().await {
        eprintln!("⚠️ Redis not available, skipping test");
        return;
    }

    let config = RedisConfig::default();
    let store = RedisStore::new(config).await.unwrap();

    let key = "test:integration:fixed_window";
    let limit = 10u64;
    let window = 3u64;

    // Clean up from previous test
    let _ = store.delete(key).await;

    // Test basic rate limiting
    for i in 1..=10 {
        let result = store.rate_limit_check(key, limit, window).await.unwrap();
        assert!(result.allowed, "Request {} should be allowed", i);
        assert_eq!(result.limit, limit);
        assert_eq!(result.remaining, limit - i);
    }

    // Should be denied now
    let result = store.rate_limit_check(key, limit, window).await.unwrap();
    assert!(!result.allowed, "11th request should be denied");
    assert_eq!(result.remaining, 0);

    // Wait for window to reset
    tokio::time::sleep(Duration::from_secs(window + 1)).await;

    // Should work again
    let result = store.rate_limit_check(key, limit, window).await.unwrap();
    assert!(result.allowed, "Request should be allowed after window reset");

    // Clean up
    let _ = store.delete(key).await;
}

#[tokio::test]
async fn test_redis_sliding_window_accuracy() {
    if !redis_available().await {
        eprintln!("⚠️ Redis not available, skipping test");
        return;
    }

    let config = RedisConfig::default();
    let store = RedisStore::new(config).await.unwrap();

    let key = "test:integration:sliding_window";
    let limit = 5u64;
    let window = 2u64;

    // Clean up
    let cleanup_key = format!("rate:sliding:{}", key);
    let _ = store.delete(&cleanup_key).await;

    // Make 5 requests (should all pass)
    for i in 1..=5 {
        let result = store.sliding_window_check(key, limit, window).await.unwrap();
        assert!(result.allowed, "Request {} should be allowed", i);
    }

    // 6th request should fail
    let result = store.sliding_window_check(key, limit, window).await.unwrap();
    assert!(!result.allowed, "6th request should be denied");

    // Wait 1 second (half the window)
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Still denied (requests still in window)
    let result = store.sliding_window_check(key, limit, window).await.unwrap();
    assert!(!result.allowed, "Request should still be denied");

    // Wait another 1.5 seconds (total 2.5 seconds - window expired)
    tokio::time::sleep(Duration::from_millis(1500)).await;

    // Should be allowed now
    let result = store.sliding_window_check(key, limit, window).await.unwrap();
    assert!(result.allowed, "Request should be allowed after window slides");

    // Clean up
    let _ = store.delete(&cleanup_key).await;
}

#[tokio::test]
async fn test_redis_session_cache_with_expiry() {
    if !redis_available().await {
        eprintln!("⚠️ Redis not available, skipping test");
        return;
    }

    let config = RedisConfig::default();
    let store = RedisStore::new(config).await.unwrap();

    let token_hash = "test_session_token_12345";
    let user_id = "user_67890";

    // Cache session with 2 second TTL
    store.cache_session(token_hash, user_id, 2).await.unwrap();

    // Should retrieve immediately
    let cached = store.get_cached_session(token_hash).await.unwrap();
    assert_eq!(cached, Some(user_id.to_string()));

    // Wait for expiry
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Should be None now
    let cached = store.get_cached_session(token_hash).await.unwrap();
    assert_eq!(cached, None, "Session should have expired");
}

#[tokio::test]
async fn test_redis_concurrent_rate_limiting() {
    if !redis_available().await {
        eprintln!("⚠️ Redis not available, skipping test");
        return;
    }

    let config = RedisConfig::default();
    let store = std::sync::Arc::new(RedisStore::new(config).await.unwrap());

    let key = "test:integration:concurrent";
    let limit = 20u64;
    let window = 5u64;

    // Clean up
    let _ = store.delete(key).await;

    // Spawn 30 concurrent requests
    let mut handles = vec![];
    for _ in 0..30 {
        let store_clone = store.clone();
        let key = key.to_string();
        let handle = tokio::spawn(async move {
            store_clone.rate_limit_check(&key, limit, window).await
        });
        handles.push(handle);
    }

    // Collect results
    let mut allowed = 0;
    let mut denied = 0;

    for handle in handles {
        match handle.await {
            Ok(Ok(result)) => {
                if result.allowed {
                    allowed += 1;
                } else {
                    denied += 1;
                }
            }
            _ => {}
        }
    }

    // Should have exactly limit allowed
    assert_eq!(allowed, limit as usize, "Should allow exactly {} requests", limit);
    assert_eq!(denied, 10, "Should deny 10 requests");

    // Clean up
    let _ = store.delete(key).await;
}

#[tokio::test]
async fn test_redis_rate_limit_store_trait_implementation() {
    if !redis_available().await {
        eprintln!("⚠️ Redis not available, skipping test");
        return;
    }

    let config = RedisConfig::default();
    let store = RedisStore::new(config).await.unwrap();

    let key = "test:integration:trait_impl";
    let window = Duration::from_secs(60);

    // Clean up
    let _ = store.reset(key);

    // Test increment
    let count1 = store.increment(key, window).unwrap();
    let count2 = store.increment(key, window).unwrap();

    assert!(count1 > 0, "First count should be > 0");
    assert!(count2 > count1, "Count should increment");

    // Test get_count
    let current = store.get_count(key).unwrap();
    assert!(current >= count2, "Current count should be >= last increment");

    // Test get_reset_time
    let reset_time = store.get_reset_time(key).unwrap();
    let now = std::time::SystemTime::now();
    assert!(reset_time > now, "Reset time should be in the future");

    // Test reset
    store.reset(key).unwrap();
    let after_reset = store.get_count(key).unwrap();
    assert_eq!(after_reset, 0, "Count should be 0 after reset");
}

#[tokio::test]
async fn test_redis_invalidate_all_user_sessions() {
    if !redis_available().await {
        eprintln!("⚠️ Redis not available, skipping test");
        return;
    }

    let config = RedisConfig::default();
    let store = RedisStore::new(config).await.unwrap();

    let user_id = "test_user_999";

    // Create 5 sessions for the same user
    for i in 1..=5 {
        let token_hash = format!("token_invalidate_test_{}", i);
        store.cache_session(&token_hash, user_id, 300).await.unwrap();
    }

    // Verify all sessions exist
    for i in 1..=5 {
        let token_hash = format!("token_invalidate_test_{}", i);
        let cached = store.get_cached_session(&token_hash).await.unwrap();
        assert_eq!(cached, Some(user_id.to_string()));
    }

    // Invalidate all user sessions
    let deleted = store.invalidate_user_sessions(user_id).await.unwrap();
    assert_eq!(deleted, 5, "Should delete all 5 sessions");

    // Verify all sessions are gone
    for i in 1..=5 {
        let token_hash = format!("token_invalidate_test_{}", i);
        let cached = store.get_cached_session(&token_hash).await.unwrap();
        assert_eq!(cached, None, "Session {} should be deleted", i);
    }
}

#[tokio::test]
async fn test_redis_key_value_operations() {
    if !redis_available().await {
        eprintln!("⚠️ Redis not available, skipping test");
        return;
    }

    let config = RedisConfig::default();
    let store = RedisStore::new(config).await.unwrap();

    let key = "test:integration:kv";
    let value = "integration_test_value";

    // Set with TTL
    store.set_with_ttl(key, value, 10).await.unwrap();

    // Get
    let retrieved = store.get(key).await.unwrap();
    assert_eq!(retrieved, Some(value.to_string()));

    // Exists
    assert!(store.exists(key).await.unwrap());

    // TTL
    let ttl = store.ttl(key).await.unwrap();
    assert!(ttl > 0 && ttl <= 10, "TTL should be between 0 and 10");

    // Delete
    assert!(store.delete(key).await.unwrap());

    // Should not exist
    assert!(!store.exists(key).await.unwrap());
}

#[tokio::test]
async fn test_redis_config_validation() {
    let valid_config = RedisConfig::default();
    assert!(valid_config.validate().is_ok());

    let empty_url_config = RedisConfig {
        url: "".to_string(),
        ..Default::default()
    };
    assert!(empty_url_config.validate().is_err());

    let zero_pool_config = RedisConfig {
        pool_size: 0,
        ..Default::default()
    };
    assert!(zero_pool_config.validate().is_err());
}

#[tokio::test]
async fn test_redis_connection_ping() {
    if !redis_available().await {
        eprintln!("⚠️ Redis not available, skipping test");
        return;
    }

    let config = RedisConfig::default();
    let store = RedisStore::new(config).await.unwrap();

    // Ping should succeed
    assert!(store.ping().await.is_ok());
}
