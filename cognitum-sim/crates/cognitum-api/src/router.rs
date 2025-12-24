//! API route configuration

use actix_web::web;
use crate::handlers;

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // Health endpoints (no auth required)
        .route("/health", web::get().to(handlers::health_check))
        .route("/metrics", web::get().to(handlers::health::metrics))
        // API v1 endpoints
        .service(
            web::scope("/api/v1")
                // Simulation endpoints
                .route("/simulations", web::post().to(handlers::create_simulation))
                .route("/simulations", web::get().to(handlers::list_simulations))
                .route("/simulations/{id}", web::get().to(handlers::get_simulation))
                .route("/simulations/{id}/run", web::post().to(handlers::run_simulation))
                .route("/simulations/{id}/status", web::get().to(handlers::get_status))
                .route("/simulations/{id}/results", web::get().to(handlers::get_results))
                .route("/simulations/{id}", web::delete().to(handlers::delete_simulation))
                .route("/simulations/{id}/stream", web::get().to(handlers::simulation_stream))
                // Program endpoints
                .route("/programs", web::post().to(handlers::create_program))
                .route("/programs/{id}", web::get().to(handlers::get_program)),
        );
}
