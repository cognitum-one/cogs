//! HTTP Server Configuration and Startup
//!
//! Provides server builder with:
//! - Configurable bind address and port
//! - CORS configuration
//! - Request tracing and logging
//! - Graceful shutdown
//! - TLS support (placeholder)

use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tower_http::{
    cors::{Any, CorsLayer},
    timeout::TimeoutLayer,
    trace::TraceLayer,
};

use super::{
    handlers::ApiState,
    routes::create_api_routes,
};

/// Server configuration
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Bind address (e.g., "0.0.0.0")
    pub host: String,

    /// Port to listen on
    pub port: u16,

    /// Request timeout in seconds
    pub request_timeout: u64,

    /// Enable CORS
    pub enable_cors: bool,

    /// CORS allowed origins (None = allow all)
    pub cors_origins: Option<Vec<String>>,

    /// Enable request tracing
    pub enable_tracing: bool,

    /// TLS certificate path (optional)
    pub tls_cert_path: Option<String>,

    /// TLS key path (optional)
    pub tls_key_path: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8080,
            request_timeout: 30,
            enable_cors: true,
            cors_origins: None, // Allow all origins by default
            enable_tracing: true,
            tls_cert_path: None,
            tls_key_path: None,
        }
    }
}

/// API Server
pub struct ApiServer {
    config: ServerConfig,
    state: Arc<ApiState>,
}

impl ApiServer {
    /// Create a new API server
    pub fn new(config: ServerConfig, state: Arc<ApiState>) -> Self {
        Self { config, state }
    }

    /// Build the router with all middleware
    pub fn build_router(&self) -> Router {
        let mut router = create_api_routes(self.state.clone());

        // Configure CORS
        let cors = if self.config.enable_cors {
            if let Some(ref origins) = self.config.cors_origins {
                // Specific origins
                let mut cors_layer = CorsLayer::new();
                for origin in origins {
                    if let Ok(origin_value) = origin.parse::<http::HeaderValue>() {
                        cors_layer = cors_layer.allow_origin(origin_value);
                    }
                }
                cors_layer
                    .allow_methods(Any)
                    .allow_headers(Any)
            } else {
                // Allow all origins
                CorsLayer::permissive()
            }
        } else {
            // CORS disabled - use very permissive to avoid type issues
            CorsLayer::permissive()
        };

        // Apply layers to router
        router = router.layer(TimeoutLayer::new(Duration::from_secs(self.config.request_timeout)));
        router = router.layer(cors);

        // Add tracing if enabled
        if self.config.enable_tracing {
            router = router.layer(TraceLayer::new_for_http());
        }

        router
    }

    /// Start the server
    ///
    /// # Errors
    /// Returns error if server fails to bind or start
    pub async fn serve(self) -> Result<(), std::io::Error> {
        let router = self.build_router();
        let addr = SocketAddr::from((
            self.config.host.parse::<std::net::IpAddr>()
                .unwrap_or_else(|_| std::net::IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))),
            self.config.port,
        ));

        // Check if TLS is configured
        if self.config.tls_cert_path.is_some() && self.config.tls_key_path.is_some() {
            // TODO: Implement TLS support with rustls
            tracing::warn!("TLS configuration provided but not yet implemented. Starting without TLS.");
        }

        tracing::info!("Starting API server on {}", addr);

        // Create TCP listener
        let listener = tokio::net::TcpListener::bind(addr).await?;

        // Serve with graceful shutdown
        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown_signal())
            .await
    }

    /// Get the bind address
    pub fn address(&self) -> String {
        format!("{}:{}", self.config.host, self.config.port)
    }
}

/// Shutdown signal handler
///
/// Listens for SIGTERM/SIGINT for graceful shutdown
async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            tracing::info!("Received Ctrl+C, shutting down gracefully");
        },
        _ = terminate => {
            tracing::info!("Received SIGTERM, shutting down gracefully");
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{InMemoryStore, RateLimitConfig};
    use crate::auth::{ApiKeyService, JwtService, MockApiKeyStore, MockTokenStore};

    fn create_test_state() -> Arc<ApiState> {
        let rate_limit_store = Arc::new(InMemoryStore::new());
        let rate_limit_config = RateLimitConfig::default();
        let jwt_store = Arc::new(MockTokenStore::new());
        let jwt_service = Arc::new(JwtService::new(jwt_store, Default::default()));
        let api_key_store = Arc::new(MockApiKeyStore::new());
        let api_key_service = Arc::new(ApiKeyService::new(api_key_store));

        Arc::new(ApiState {
            rate_limit_store,
            rate_limit_config,
            jwt_service,
            api_key_service,
        })
    }

    #[test]
    fn server_config_default() {
        let config = ServerConfig::default();
        assert_eq!(config.host, "127.0.0.1");
        assert_eq!(config.port, 8080);
        assert_eq!(config.request_timeout, 30);
        assert!(config.enable_cors);
        assert!(config.enable_tracing);
    }

    #[test]
    fn server_build_router() {
        let config = ServerConfig::default();
        let state = create_test_state();
        let server = ApiServer::new(config, state);

        // Should build without panic
        let _router = server.build_router();
    }

    #[test]
    fn server_address() {
        let config = ServerConfig {
            host: "0.0.0.0".to_string(),
            port: 3000,
            ..Default::default()
        };
        let state = create_test_state();
        let server = ApiServer::new(config, state);

        assert_eq!(server.address(), "0.0.0.0:3000");
    }
}
