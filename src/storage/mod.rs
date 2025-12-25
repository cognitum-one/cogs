//! Storage layer for Cognitum
//!
//! Provides PostgreSQL-backed storage for users, sessions, API keys, and audit logs
//! with optional Redis caching support.

pub mod postgres;
pub mod redis;
pub mod migrations;

pub use postgres::{PostgresStore, PostgresConfig, UserRecord, ApiKeyRecord, RefreshTokenRecord, AuditEventRecord};
pub use redis::{RedisStore, RedisConfig, RateLimitResult};

use thiserror::Error;

/// Storage errors
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("API key not found: {0}")]
    ApiKeyNotFound(String),

    #[error("Refresh token not found: {0}")]
    RefreshTokenNotFound(String),

    #[error("User already exists: {0}")]
    UserAlreadyExists(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Connection pool error: {0}")]
    PoolError(String),

    #[error("Redis error: {0}")]
    Redis(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type StorageResult<T> = Result<T, StorageError>;
