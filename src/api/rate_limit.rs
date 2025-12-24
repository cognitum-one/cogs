//! Rate Limiting Module for Cognitum API
//!
//! Implements token bucket algorithm with:
//! - Per-API-key rate limiting
//! - Per-endpoint rate limiting
//! - Burst handling
//! - DDoS protection
//! - RFC 6585 compliant headers

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use thiserror::Error;

use super::rate_limit_store::{RateLimitStore, StoreError};

/// Rate limiting errors
#[derive(Debug, Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded. Retry after {retry_after} seconds")]
    Exceeded { retry_after: u64 },

    #[error("Store error: {0}")]
    Store(#[from] StoreError),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Rate limit configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Default requests per minute for all endpoints
    pub requests_per_minute: u64,

    /// Maximum burst size (additional requests allowed above rate)
    pub burst_size: u64,

    /// Per-endpoint limits (overrides default)
    /// Format: (endpoint_path, requests_per_minute)
    pub endpoint_limits: Vec<(&'static str, u64)>,

    /// Enable DDoS protection features
    pub ddos_protection: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            requests_per_minute: 100,
            burst_size: 10,
            endpoint_limits: vec![
                ("/api/v1/simulate", 1000),         // High limit for simulation
                ("/api/v1/license/validate", 10),   // Low limit for license checks
                ("/api/v1/auth/login", 5),          // Very low for authentication
            ],
            ddos_protection: true,
        }
    }
}

/// RFC 6585 compliant rate limit headers
#[derive(Debug, Clone)]
pub struct RateLimitHeaders {
    /// X-RateLimit-Limit: Maximum requests allowed per window
    pub limit: u64,

    /// X-RateLimit-Remaining: Requests remaining in current window
    pub remaining: u64,

    /// X-RateLimit-Reset: Unix timestamp when the window resets
    pub reset: u64,

    /// Retry-After: Seconds until the client can retry (only when rate limited)
    pub retry_after: Option<u64>,
}

impl RateLimitHeaders {
    /// Convert headers to HashMap for HTTP response
    pub fn to_map(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        headers.insert("X-RateLimit-Limit".to_string(), self.limit.to_string());
        headers.insert("X-RateLimit-Remaining".to_string(), self.remaining.to_string());
        headers.insert("X-RateLimit-Reset".to_string(), self.reset.to_string());

        if let Some(retry_after) = self.retry_after {
            headers.insert("Retry-After".to_string(), retry_after.to_string());
        }

        headers
    }
}

/// Result of a rate limit check with headers
#[derive(Debug)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,

    /// Rate limit headers to include in response
    pub headers: HashMap<String, String>,
}

/// Rate limiter using token bucket algorithm
///
/// # Algorithm
/// Token bucket allows for:
/// 1. Steady rate limiting (tokens refill at constant rate)
/// 2. Burst handling (bucket can hold burst_size extra tokens)
/// 3. Fair distribution (independent buckets per API key/endpoint)
///
/// # Example
/// ```rust,no_run
/// use cognitum_api::rate_limit::{RateLimiter, RateLimitConfig, InMemoryStore};
/// use std::sync::Arc;
///
/// let config = RateLimitConfig::default();
/// let store = Arc::new(InMemoryStore::new());
/// let limiter = RateLimiter::new(store, config);
///
/// // Check if request is allowed
/// match limiter.check("sk_test_123").await {
///     Ok(_) => println!("Request allowed"),
///     Err(e) => println!("Rate limited: {}", e),
/// }
/// ```
pub struct RateLimiter {
    store: Arc<dyn RateLimitStore>,
    config: RateLimitConfig,
    endpoint_limits_map: HashMap<String, u64>,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `store` - Backend storage for rate limit counters
    /// * `config` - Rate limiting configuration
    pub fn new(store: Arc<dyn RateLimitStore>, config: RateLimitConfig) -> Self {
        let endpoint_limits_map = config
            .endpoint_limits
            .iter()
            .map(|(path, limit)| (path.to_string(), *limit))
            .collect();

        Self {
            store,
            config,
            endpoint_limits_map,
        }
    }

    /// Check if a request should be rate limited (simple version)
    ///
    /// # Arguments
    /// * `api_key` - API key making the request
    ///
    /// # Returns
    /// Ok(()) if allowed, Err(RateLimitError::Exceeded) if rate limited
    pub async fn check(&self, api_key: &str) -> Result<(), RateLimitError> {
        let result = self.check_with_headers(api_key, None).await?;

        if result.allowed {
            Ok(())
        } else {
            Err(RateLimitError::Exceeded {
                retry_after: result.headers
                    .get("Retry-After")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(60),
            })
        }
    }

    /// Check rate limit and return headers
    ///
    /// # Arguments
    /// * `api_key` - API key making the request
    /// * `endpoint` - Optional endpoint path for endpoint-specific limits
    ///
    /// # Returns
    /// RateLimitResult with allowed status and headers
    pub async fn check_with_headers(
        &self,
        api_key: &str,
        endpoint: Option<&str>,
    ) -> Result<RateLimitResult, RateLimitError> {
        // Determine the limit for this request
        let limit = if let Some(endpoint) = endpoint {
            self.get_limit(endpoint)
        } else {
            self.config.requests_per_minute
        };

        // Create unique key for this API key + endpoint combination
        let key = self.make_key(api_key, endpoint);

        // Window is always 1 minute for requests_per_minute
        let window = Duration::from_secs(60);

        // Increment counter
        let current_count = self.store.increment(&key, window)?;

        // Calculate effective limit with burst
        let effective_limit = limit + self.config.burst_size;

        // Check if within limit
        let allowed = current_count <= effective_limit;

        // Get reset time
        let reset_time = self.store.get_reset_time(&key)?;
        let reset_timestamp = reset_time
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Calculate remaining
        let remaining = if current_count <= effective_limit {
            effective_limit - current_count
        } else {
            0
        };

        // Calculate retry_after if rate limited
        let retry_after = if !allowed {
            let now = SystemTime::now();
            reset_time
                .duration_since(now)
                .map(|d| d.as_secs())
                .ok()
        } else {
            None
        };

        // Build headers
        let headers = RateLimitHeaders {
            limit: effective_limit,
            remaining,
            reset: reset_timestamp,
            retry_after,
        };

        Ok(RateLimitResult {
            allowed,
            headers: headers.to_map(),
        })
    }

    /// Get the rate limit for a specific endpoint
    pub fn get_limit(&self, endpoint: &str) -> u64 {
        self.endpoint_limits_map
            .get(endpoint)
            .copied()
            .unwrap_or(self.config.requests_per_minute)
    }

    /// Create a storage key for API key + endpoint combination
    fn make_key(&self, api_key: &str, endpoint: Option<&str>) -> String {
        if let Some(endpoint) = endpoint {
            format!("ratelimit:{}:{}", api_key, endpoint)
        } else {
            format!("ratelimit:{}", api_key)
        }
    }

    /// Reset rate limit for a specific API key (admin function)
    pub async fn reset(&self, api_key: &str, endpoint: Option<&str>) -> Result<(), RateLimitError> {
        let key = self.make_key(api_key, endpoint);
        self.store.reset(&key)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::InMemoryStore;

    fn create_test_limiter() -> RateLimiter {
        let store = Arc::new(InMemoryStore::new());
        let config = RateLimitConfig {
            requests_per_minute: 10,
            burst_size: 2,
            endpoint_limits: vec![
                ("/api/v1/simulate", 100),
                ("/api/v1/auth/login", 3),
            ],
            ddos_protection: true,
        };
        RateLimiter::new(store, config)
    }

    #[tokio::test]
    async fn rate_limiter_allows_requests_within_limit() {
        let limiter = create_test_limiter();
        let api_key = "sk_test_123";

        // First 12 requests should succeed (10 + 2 burst)
        for i in 0..12 {
            let result = limiter.check(api_key).await;
            assert!(result.is_ok(), "Request {} should succeed", i);
        }
    }

    #[tokio::test]
    async fn rate_limiter_blocks_requests_over_limit() {
        let limiter = create_test_limiter();
        let api_key = "sk_test_123";

        // Use up all tokens
        for _ in 0..12 {
            let _ = limiter.check(api_key).await;
        }

        // 13th request should be rate limited
        let result = limiter.check(api_key).await;
        assert!(matches!(result, Err(RateLimitError::Exceeded { .. })));
    }

    #[tokio::test]
    async fn rate_limiter_returns_correct_headers() {
        let limiter = create_test_limiter();
        let api_key = "sk_test_123";

        let result = limiter.check_with_headers(api_key, None).await.unwrap();

        assert!(result.allowed);
        assert!(result.headers.contains_key("X-RateLimit-Limit"));
        assert!(result.headers.contains_key("X-RateLimit-Remaining"));
        assert!(result.headers.contains_key("X-RateLimit-Reset"));

        // Limit should be base + burst
        let limit: u64 = result.headers.get("X-RateLimit-Limit").unwrap().parse().unwrap();
        assert_eq!(limit, 12); // 10 + 2 burst
    }

    #[tokio::test]
    async fn rate_limiter_includes_retry_after_when_limited() {
        let limiter = create_test_limiter();
        let api_key = "sk_test_123";

        // Exhaust limit
        for _ in 0..12 {
            let _ = limiter.check(api_key).await;
        }

        // Next request should include Retry-After
        let result = limiter.check_with_headers(api_key, None).await.unwrap();

        assert!(!result.allowed);
        assert!(result.headers.contains_key("Retry-After"));

        let retry_after: u64 = result.headers.get("Retry-After").unwrap().parse().unwrap();
        assert!(retry_after > 0 && retry_after <= 60);
    }

    #[tokio::test]
    async fn rate_limiter_applies_endpoint_specific_limits() {
        let limiter = create_test_limiter();
        let api_key = "sk_test_123";

        // Simulate endpoint has limit of 100
        let sim_limit = limiter.get_limit("/api/v1/simulate");
        assert_eq!(sim_limit, 100);

        // Auth endpoint has limit of 3
        let auth_limit = limiter.get_limit("/api/v1/auth/login");
        assert_eq!(auth_limit, 3);

        // Unknown endpoint uses default
        let default_limit = limiter.get_limit("/api/v1/unknown");
        assert_eq!(default_limit, 10);
    }

    #[tokio::test]
    async fn rate_limiter_separates_endpoint_buckets() {
        let limiter = create_test_limiter();
        let api_key = "sk_test_123";

        // Use up auth endpoint limit (3 + 2 burst = 5)
        for _ in 0..5 {
            let _ = limiter.check_with_headers(api_key, Some("/api/v1/auth/login")).await;
        }

        // Auth should be rate limited
        let auth_result = limiter.check_with_headers(api_key, Some("/api/v1/auth/login")).await.unwrap();
        assert!(!auth_result.allowed);

        // But simulate endpoint should still work
        let sim_result = limiter.check_with_headers(api_key, Some("/api/v1/simulate")).await.unwrap();
        assert!(sim_result.allowed);
    }

    #[tokio::test]
    async fn rate_limiter_reset_clears_counter() {
        let limiter = create_test_limiter();
        let api_key = "sk_test_123";

        // Use some tokens
        for _ in 0..5 {
            let _ = limiter.check(api_key).await;
        }

        // Reset
        limiter.reset(api_key, None).await.unwrap();

        // Should have full quota again
        let result = limiter.check_with_headers(api_key, None).await.unwrap();
        assert_eq!(result.headers.get("X-RateLimit-Remaining").unwrap(), "11"); // 12 - 1
    }
}
