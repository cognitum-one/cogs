//! Health check handler

use actix_web::{web, HttpResponse};
use crate::models::response::HealthResponse;
use std::time::SystemTime;

lazy_static::lazy_static! {
    static ref START_TIME: SystemTime = SystemTime::now();
}

/// GET /health - Health check endpoint
pub async fn health_check() -> HttpResponse {
    let uptime = START_TIME
        .elapsed()
        .map(|d| d.as_secs())
        .unwrap_or(0);

    HttpResponse::Ok().json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
    })
}

/// GET /metrics - Prometheus metrics endpoint
pub async fn metrics() -> HttpResponse {
    // TODO: Implement Prometheus metrics collection
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("# Cognitum API Metrics\n# TODO: Implement metrics\n")
}
