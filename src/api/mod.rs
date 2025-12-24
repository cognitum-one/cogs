//! Cognitum API Module
//!
//! Provides API infrastructure including:
//! - Rate limiting
//! - Authentication
//! - Request validation
//! - Response formatting

pub mod rate_limit;
pub mod rate_limit_store;

pub use rate_limit::{
    RateLimiter,
    RateLimitConfig,
    RateLimitError,
    RateLimitHeaders,
    RateLimitResult,
};

pub use rate_limit_store::{
    RateLimitStore,
    InMemoryStore,
    RedisStore,
    StoreError,
};
