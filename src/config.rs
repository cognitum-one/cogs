//! Configuration module for loading environment variables
//!
//! Provides centralized configuration management for the Cognitum API

use std::env;
use std::time::Duration;

/// Application configuration loaded from environment variables
#[derive(Debug, Clone)]
pub struct Config {
    /// Database configuration
    pub database: DatabaseConfig,

    /// Redis configuration
    pub redis: RedisConfig,

    /// JWT configuration
    pub jwt: JwtConfig,

    /// Server configuration
    pub server: ServerConfig,

    /// Rate limiting configuration
    pub rate_limit: RateLimitConfig,

    /// Feature flags
    pub features: FeatureFlags,
}

/// Database configuration
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// PostgreSQL connection URL
    pub url: String,

    /// Maximum number of connections in the pool
    pub max_connections: u32,

    /// Connection timeout in seconds
    pub connection_timeout: Duration,
}

/// Redis configuration
#[derive(Debug, Clone)]
pub struct RedisConfig {
    /// Redis connection URL
    pub url: String,

    /// Connection pool size
    pub pool_size: usize,

    /// Connection timeout in seconds
    pub timeout: Duration,
}

/// JWT configuration
#[derive(Debug, Clone)]
pub struct JwtConfig {
    /// Secret key for signing tokens
    pub secret: String,

    /// Access token TTL in seconds
    pub access_token_ttl: u64,

    /// Refresh token TTL in seconds
    pub refresh_token_ttl: u64,
}

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Host to bind to
    pub host: String,

    /// Port to listen on
    pub port: u16,

    /// Request timeout in seconds
    pub request_timeout: u64,

    /// Enable CORS
    pub enable_cors: bool,

    /// CORS allowed origins
    pub cors_origins: Option<Vec<String>>,
}

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Requests per minute
    pub requests_per_minute: u32,

    /// Burst size
    pub burst: u32,
}

/// Feature flags
#[derive(Debug, Clone)]
pub struct FeatureFlags {
    /// Enable metrics collection
    pub enable_metrics: bool,

    /// Enable request tracing
    pub enable_tracing: bool,

    /// Enable API documentation
    pub enable_api_docs: bool,
}

/// Load configuration from environment variables
///
/// # Errors
/// Returns error if required environment variables are missing or invalid
pub fn load_from_env() -> Result<Config, ConfigError> {
    Ok(Config {
        database: DatabaseConfig {
            url: env::var("DATABASE_URL")
                .map_err(|_| ConfigError::MissingVar("DATABASE_URL"))?,
            max_connections: env::var("DATABASE_MAX_CONNECTIONS")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("DATABASE_MAX_CONNECTIONS"))?,
            connection_timeout: Duration::from_secs(
                env::var("DATABASE_CONNECTION_TIMEOUT")
                    .unwrap_or_else(|_| "30".to_string())
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue("DATABASE_CONNECTION_TIMEOUT"))?
            ),
        },

        redis: RedisConfig {
            url: env::var("REDIS_URL")
                .map_err(|_| ConfigError::MissingVar("REDIS_URL"))?,
            pool_size: env::var("REDIS_POOL_SIZE")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("REDIS_POOL_SIZE"))?,
            timeout: Duration::from_secs(
                env::var("REDIS_TIMEOUT")
                    .unwrap_or_else(|_| "5".to_string())
                    .parse()
                    .map_err(|_| ConfigError::InvalidValue("REDIS_TIMEOUT"))?
            ),
        },

        jwt: JwtConfig {
            secret: env::var("JWT_SECRET")
                .map_err(|_| ConfigError::MissingVar("JWT_SECRET"))?,
            access_token_ttl: env::var("JWT_ACCESS_TOKEN_TTL")
                .unwrap_or_else(|_| "900".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("JWT_ACCESS_TOKEN_TTL"))?,
            refresh_token_ttl: env::var("JWT_REFRESH_TOKEN_TTL")
                .unwrap_or_else(|_| "604800".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("JWT_REFRESH_TOKEN_TTL"))?,
        },

        server: ServerConfig {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            port: env::var("PORT")
                .unwrap_or_else(|_| "8080".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("PORT"))?,
            request_timeout: env::var("REQUEST_TIMEOUT")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("REQUEST_TIMEOUT"))?,
            enable_cors: env::var("ENABLE_CORS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            cors_origins: env::var("CORS_ALLOWED_ORIGINS")
                .ok()
                .map(|s| s.split(',').map(|s| s.trim().to_string()).collect()),
        },

        rate_limit: RateLimitConfig {
            requests_per_minute: env::var("RATE_LIMIT_REQUESTS_PER_MINUTE")
                .unwrap_or_else(|_| "100".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("RATE_LIMIT_REQUESTS_PER_MINUTE"))?,
            burst: env::var("RATE_LIMIT_BURST")
                .unwrap_or_else(|_| "20".to_string())
                .parse()
                .map_err(|_| ConfigError::InvalidValue("RATE_LIMIT_BURST"))?,
        },

        features: FeatureFlags {
            enable_metrics: env::var("ENABLE_METRICS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_tracing: env::var("ENABLE_TRACING")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
            enable_api_docs: env::var("ENABLE_API_DOCS")
                .unwrap_or_else(|_| "true".to_string())
                .parse()
                .unwrap_or(true),
        },
    })
}

/// Configuration error
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Missing environment variable: {0}")]
    MissingVar(&'static str),

    #[error("Invalid value for environment variable: {0}")]
    InvalidValue(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        // Set required env vars
        env::set_var("DATABASE_URL", "postgres://test:test@localhost/test");
        env::set_var("REDIS_URL", "redis://localhost:6379");
        env::set_var("JWT_SECRET", "test-secret");

        let config = load_from_env().expect("Failed to load config");

        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.rate_limit.requests_per_minute, 100);
        assert!(config.features.enable_metrics);
    }
}
