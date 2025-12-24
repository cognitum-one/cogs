//! Cognitum REST API Server
//!
//! Provides HTTP/WebSocket API for the Cognitum neuromorphic simulator.
//!
//! ## Features
//! - REST API for simulation management
//! - WebSocket streaming for real-time events
//! - API key authentication
//! - Rate limiting
//! - CORS support

pub mod config;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod router;
pub mod services;
pub mod queue;

use actix_web::{web, App, HttpServer};
use std::sync::Arc;

pub use config::ApiConfig;
pub use models::{AppState, error::ApiError};

/// Configure the application routes and middleware
pub fn configure_app(cfg: &mut web::ServiceConfig) {
    router::configure_routes(cfg);
}

/// Create and run the API server
pub async fn run_server(config: ApiConfig, state: AppState) -> std::io::Result<()> {
    let bind_addr = format!("{}:{}", config.host, config.port);
    tracing::info!("Starting Cognitum API server on {}", bind_addr);

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(state.clone()))
            .configure(configure_app)
    })
    .bind(&bind_addr)?
    .run()
    .await
}
