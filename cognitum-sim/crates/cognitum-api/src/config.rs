//! API server configuration

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Server host address
    pub host: String,

    /// Server port
    pub port: u16,

    /// API version prefix
    pub api_version: String,

    /// Enable CORS
    pub enable_cors: bool,

    /// CORS allowed origins
    pub cors_origins: Vec<String>,

    /// JWT secret for token validation
    pub jwt_secret: String,

    /// Rate limit: requests per minute
    pub rate_limit_rpm: u32,

    /// Enable metrics endpoint
    pub enable_metrics: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            api_version: "v1".to_string(),
            enable_cors: true,
            cors_origins: vec!["*".to_string()],
            jwt_secret: "development-secret-change-in-production".to_string(),
            rate_limit_rpm: 100,
            enable_metrics: true,
        }
    }
}

impl ApiConfig {
    /// Create configuration from environment variables
    pub fn from_env() -> Self {
        Self {
            host: std::env::var("API_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("API_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            api_version: std::env::var("API_VERSION").unwrap_or_else(|_| "v1".to_string()),
            enable_cors: std::env::var("ENABLE_CORS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            cors_origins: std::env::var("CORS_ORIGINS")
                .map(|s| s.split(',').map(String::from).collect())
                .unwrap_or_else(|_| vec!["*".to_string()]),
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| "development-secret-change-in-production".to_string()),
            rate_limit_rpm: std::env::var("RATE_LIMIT_RPM")
                .ok()
                .and_then(|r| r.parse().ok())
                .unwrap_or(100),
            enable_metrics: std::env::var("ENABLE_METRICS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
        }
    }
}
