//! REST API Routes for Cognitum chip v1 commercialization
//!
//! Provides HTTP endpoints for:
//! - Authentication (JWT tokens, API keys)
//! - Simulator operations
//! - Vector search (Ruvector)
//!
//! All routes are protected with authentication and rate limiting middleware.

use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;

use super::{
    handlers::{auth_handlers, ruvector_handlers, simulator_handlers, ApiState},
    middleware::auth_middleware,
};

/// Create the main API router with all v1 endpoints
///
/// # Routes
///
/// ## Authentication
/// - `POST /v1/auth/login` - Get JWT access and refresh tokens
/// - `POST /v1/auth/refresh` - Refresh access token using refresh token
/// - `POST /v1/keys` - Create new API key
/// - `DELETE /v1/keys/:id` - Revoke API key
///
/// ## Simulator
/// - `POST /v1/simulator/execute` - Execute simulation
/// - `GET /v1/simulator/status` - Get simulator status
///
/// ## Ruvector (Vector Search)
/// - `POST /v1/ruvector/search` - Vector similarity search
/// - `POST /v1/ruvector/insert` - Insert vectors into index
///
/// # Security
///
/// All endpoints except authentication require:
/// - Valid JWT token in `Authorization: Bearer <token>` header, OR
/// - Valid API key in `X-API-Key` header
///
/// All endpoints are rate-limited per the configured limits.
pub fn create_api_routes(state: Arc<ApiState>) -> Router {
    Router::new()
        // Authentication routes (no auth required)
        .route("/v1/auth/login", post(auth_handlers::login))
        .route("/v1/auth/refresh", post(auth_handlers::refresh_token))

        // Protected routes (authentication required)
        .route("/v1/keys", post(auth_handlers::create_api_key))
        .route("/v1/keys/:id", delete(auth_handlers::revoke_api_key))
        .route("/v1/simulator/execute", post(simulator_handlers::execute_simulation))
        .route("/v1/simulator/status", get(simulator_handlers::get_status))
        .route("/v1/ruvector/search", post(ruvector_handlers::vector_search))
        .route("/v1/ruvector/insert", post(ruvector_handlers::insert_vectors))

        // Apply authentication middleware to all routes except /auth/login and /auth/refresh
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))

        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{InMemoryStore, RateLimitConfig};
    use crate::auth::{ApiKeyService, JwtService, MockApiKeyStore, MockTokenStore};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    async fn create_test_state() -> Arc<ApiState> {
        let rate_limit_store = Arc::new(InMemoryStore::new());
        let rate_limit_config = RateLimitConfig::default();
        let jwt_store = Arc::new(MockTokenStore::new());
        let jwt_service = Arc::new(JwtService::new(jwt_store, Default::default()));
        let api_key_store = Arc::new(MockApiKeyStore::new());
        let api_key_service = Arc::new(ApiKeyService::new(api_key_store));

        // Create simulator
        let simulator_config = crate::sdk::types::SimulatorConfig::default();
        let simulator = crate::sdk::core::CognitumSimulator::new(simulator_config)
            .expect("Failed to create simulator");

        // Create ruvector
        let ruvector_config = crate::ruvector::types::RuvectorConfig::default();
        let ruvector = crate::ruvector::facade::CognitumRuvector::new(ruvector_config);

        // Create user store
        let user_store = Arc::new(crate::api::handlers::UserStore::new());

        Arc::new(ApiState {
            rate_limit_store,
            rate_limit_config,
            jwt_service,
            api_key_service,
            user_store,
            simulator: Arc::new(tokio::sync::Mutex::new(simulator)),
            ruvector: Arc::new(ruvector),
        })
    }

    #[tokio::test]
    async fn login_endpoint_exists() {
        let state = create_test_state().await;
        let app = create_api_routes(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/auth/login")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"username":"test","password":"test"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should not return 404
        assert_ne!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn unknown_route_returns_404() {
        let state = create_test_state().await;
        let app = create_api_routes(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/unknown")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
