//! Cognitum API Server Binary
//!
//! Main entry point for the Cognitum HTTP API server.
//!
//! This binary:
//! - Loads configuration from environment variables
//! - Initializes logging and tracing
//! - Sets up the API server with all middleware
//! - Starts the HTTP server with graceful shutdown
//!
//! NOTE: This is a minimal binary for Docker deployment.
//! For full functionality, configure database-backed stores.

use cognitum::api::{ApiServer, ServerConfig};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber for logging
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,cognitum=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("Starting Cognitum API server");

    // Load configuration from environment
    let config = load_config_from_env()?;

    tracing::info!(
        "Configuration loaded - Server: {}:{}",
        config.server.host,
        config.server.port
    );

    // TODO: Initialize storage backends
    // - PostgreSQL connection pool
    // - Redis connection for caching and rate limiting
    // - Authentication stores (JWT, API keys)
    // - Simulator and Ruvector instances

    // For now, create a minimal health-check-only server
    let addr = SocketAddr::from((
        config.server.host.parse::<std::net::IpAddr>()
            .unwrap_or_else(|_| std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0))),
        config.server.port,
    ));

    tracing::info!("Starting minimal API server on {}", addr);
    tracing::warn!("This is a minimal deployment - configure database backends for full functionality");

    // Create minimal router with health check
    let app = axum::Router::new()
        .route("/health", axum::routing::get(health_check))
        .layer(tower_http::trace::TraceLayer::new_for_http());

    tracing::info!("Server listening on {}", addr);

    // Create TCP listener and serve
    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("Server shutdown complete");
    Ok(())
}

/// Health check endpoint
async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "service": "cognitum-api",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// Shutdown signal handler
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

/// Application configuration
struct AppConfig {
    server: ServerConfig,
}

/// Load configuration from environment variables
fn load_config_from_env() -> Result<AppConfig, Box<dyn std::error::Error>> {
    let host = std::env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()?;

    let request_timeout = std::env::var("REQUEST_TIMEOUT")
        .unwrap_or_else(|_| "30".to_string())
        .parse::<u64>()?;

    Ok(AppConfig {
        server: ServerConfig {
            host,
            port,
            request_timeout,
            enable_cors: true,
            cors_origins: std::env::var("CORS_ALLOWED_ORIGINS")
                .ok()
                .map(|s| s.split(',').map(|s| s.trim().to_string()).collect()),
            enable_tracing: true,
            tls_cert_path: std::env::var("TLS_CERT_PATH").ok(),
            tls_key_path: std::env::var("TLS_KEY_PATH").ok(),
        },
    })
}
