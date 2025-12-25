//! Cognitum API Module
//!
//! Provides API infrastructure including:
//! - Rate limiting
//! - Authentication
//! - Request validation
//! - Response formatting
//! - REST API endpoints
//! - HTTP server

pub mod rate_limit;
pub mod rate_limit_store;
pub mod routes;
pub mod handlers;
pub mod middleware;
pub mod server;

// Re-export rate limiting
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

// Re-export API components
pub use handlers::{
    ApiState,
    ErrorResponse,
    SuccessResponse,
};

pub use middleware::{
    AuthenticatedUser,
    AuthMethod,
};

pub use server::{
    ApiServer,
    ServerConfig,
};
