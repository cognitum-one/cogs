//! Cognitum API Server Main Entry Point

use cognitum_api::{run_server, ApiConfig, AppState};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cognitum_api=info,actix_web=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config = ApiConfig::from_env();

    // TODO: Initialize services with real implementations
    // For now, this is a skeleton that compiles

    tracing::info!("Cognitum API server configuration loaded");
    tracing::info!("Server would start on {}:{}", config.host, config.port);
    tracing::info!("Note: Service implementations need to be provided");

    // In production, initialize AppState with real service implementations:
    // let state = AppState::new(
    //     Arc::new(SimulatorServiceImpl::new()),
    //     Arc::new(StorageServiceImpl::new()),
    //     Arc::new(AuthServiceImpl::new()),
    //     Arc::new(RateLimiterImpl::new()),
    // );
    //
    // run_server(config, state).await

    Ok(())
}
