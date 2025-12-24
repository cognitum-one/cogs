//! Redis Rate Limiting Example
//!
//! Demonstrates how to use Redis-backed rate limiting in Cognitum.
//!
//! # Prerequisites
//! - Redis server running on localhost:6379
//! - Or set REDIS_URL environment variable
//!
//! # Running
//! ```bash
//! cargo run --example redis_rate_limit_example
//! ```

use cognitum::storage::{RedisStore, RedisConfig, RateLimitResult};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Redis Rate Limiting Example ===\n");

    // 1. Configuration
    println!("1. Configuring Redis connection...");
    let redis_url = std::env::var("REDIS_URL")
        .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());

    let config = RedisConfig::new(redis_url.clone())
        .with_pool_size(10)
        .with_connect_timeout(5);

    println!("   Redis URL: {}", redis_url);
    println!("   Pool size: {}", config.pool_size);

    // 2. Create store
    println!("\n2. Creating Redis store...");
    let store = match RedisStore::new(config).await {
        Ok(store) => {
            println!("   ✅ Connected successfully");
            store
        }
        Err(e) => {
            eprintln!("   ❌ Failed to connect: {}", e);
            eprintln!("\n   Make sure Redis is running:");
            eprintln!("   docker run -d -p 6379:6379 redis:latest");
            return Err(e.into());
        }
    };

    // 3. Health check
    println!("\n3. Checking connection health...");
    match store.ping().await {
        Ok(_) => println!("   ✅ Connection healthy"),
        Err(e) => {
            eprintln!("   ❌ Health check failed: {}", e);
            return Err(e.into());
        }
    }

    // 4. Fixed Window Rate Limiting
    println!("\n4. Fixed Window Rate Limiting");
    println!("   Limit: 5 requests per 10 seconds");

    let key = "example:fixed_window";
    let limit = 5u64;
    let window = 10u64;

    for i in 1..=7 {
        let result = store.rate_limit_check(key, limit, window).await?;

        print!("   Request {}: ", i);
        if result.allowed {
            println!("✅ ALLOWED (remaining: {})", result.remaining);
        } else {
            println!("❌ RATE LIMITED (reset in {} seconds)",
                result.reset_at - (chrono::Utc::now().timestamp() as u64));
        }

        // Small delay between requests
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Clean up
    store.delete(key).await?;

    // 5. Sliding Window Rate Limiting
    println!("\n5. Sliding Window Rate Limiting");
    println!("   Limit: 3 requests per 5 seconds");

    let key = "example:sliding_window";
    let limit = 3u64;
    let window = 5u64;

    for i in 1..=5 {
        let result = store.sliding_window_check(key, limit, window).await?;

        print!("   Request {}: ", i);
        if result.allowed {
            println!("✅ ALLOWED (remaining: {})", result.remaining);
        } else {
            println!("❌ RATE LIMITED (reset at: {})", result.reset_at);
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Clean up
    let cleanup_key = format!("rate:sliding:{}", key);
    store.delete(&cleanup_key).await?;

    // 6. Session Caching
    println!("\n6. Session Caching");

    let token_hash = "example_token_abc123";
    let user_id = "user_42";
    let ttl = 60u64; // 1 minute

    println!("   Caching session for user: {}", user_id);
    store.cache_session(token_hash, user_id, ttl).await?;

    println!("   Retrieving session...");
    match store.get_cached_session(token_hash).await? {
        Some(cached_user_id) => {
            println!("   ✅ Found session for user: {}", cached_user_id);
        }
        None => {
            println!("   ❌ Session not found");
        }
    }

    println!("   Invalidating session...");
    store.invalidate_session(token_hash).await?;

    match store.get_cached_session(token_hash).await? {
        Some(_) => println!("   ❌ Session still exists (unexpected)"),
        None => println!("   ✅ Session invalidated successfully"),
    }

    // 7. Bulk Session Management
    println!("\n7. Bulk Session Management");

    let user_id = "user_99";
    println!("   Creating 3 sessions for user: {}", user_id);

    for i in 1..=3 {
        let token = format!("bulk_token_{}", i);
        store.cache_session(&token, user_id, 60).await?;
        println!("   Created session: {}", token);
    }

    println!("   Invalidating all sessions for user...");
    let deleted = store.invalidate_user_sessions(user_id).await?;
    println!("   ✅ Deleted {} sessions", deleted);

    // 8. Generic Key-Value Operations
    println!("\n8. Generic Key-Value Operations");

    let key = "example:kv:key";
    let value = "example_value";

    println!("   Setting key with 30 second TTL...");
    store.set_with_ttl(key, value, 30).await?;

    println!("   Getting value...");
    match store.get(key).await? {
        Some(retrieved) => println!("   ✅ Retrieved: {}", retrieved),
        None => println!("   ❌ Key not found"),
    }

    println!("   Checking TTL...");
    let ttl = store.ttl(key).await?;
    println!("   TTL: {} seconds", ttl);

    println!("   Deleting key...");
    if store.delete(key).await? {
        println!("   ✅ Key deleted");
    } else {
        println!("   ❌ Key not found");
    }

    // 9. Performance Test
    println!("\n9. Quick Performance Test");

    let key = "example:perf_test";
    let limit = 1000u64;
    let window = 60u64;

    println!("   Running 100 rate limit checks...");
    let start = std::time::Instant::now();

    for _ in 0..100 {
        store.rate_limit_check(key, limit, window).await?;
    }

    let elapsed = start.elapsed();
    let ops_per_sec = 100.0 / elapsed.as_secs_f64();

    println!("   Completed in: {:?}", elapsed);
    println!("   Operations/sec: {:.0}", ops_per_sec);

    // Clean up
    store.delete(key).await?;

    println!("\n=== Example completed successfully! ===");

    Ok(())
}
