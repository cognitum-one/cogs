//! Authentication and Rate Limiting Middleware
//!
//! Provides middleware layers for:
//! - JWT token validation
//! - API key validation
//! - Rate limiting per API key/endpoint
//! - Request/response logging

use axum::{
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::sync::Arc;

use super::handlers::{ApiState, ErrorResponse};
use crate::api::RateLimiter;
use crate::auth::AuthError;
use crate::auth::types::UserId;

/// Extension type to store authenticated user information in request
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: UserId,
    pub auth_method: AuthMethod,
}

/// Authentication method used
#[derive(Debug, Clone)]
pub enum AuthMethod {
    JwtToken,
    ApiKey,
}

/// Authentication middleware
///
/// Validates requests using either:
/// 1. Bearer token in Authorization header (JWT)
/// 2. API key in X-API-Key header
///
/// Sets request extension with authenticated user information.
pub async fn auth_middleware(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, ErrorResponse> {
    // Skip authentication for login and refresh endpoints
    let path = request.uri().path().to_string();
    if path == "/v1/auth/login" || path == "/v1/auth/refresh" {
        return Ok(next.run(request).await);
    }

    // Try JWT authentication first
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                match state.jwt_service.decode_access_token(token) {
                    Ok(claims) => {
                        let user_id = UserId::new(claims.user_id);
                        request.extensions_mut().insert(AuthenticatedUser {
                            user_id: user_id.clone(),
                            auth_method: AuthMethod::JwtToken,
                        });

                        // Apply rate limiting
                        return apply_rate_limiting(state, user_id, &path, request, next).await;
                    }
                    Err(AuthError::TokenExpired) => {
                        return Err(ErrorResponse::new("Access token expired", "TOKEN_EXPIRED"));
                    }
                    Err(AuthError::InvalidSignature) | Err(AuthError::MalformedToken) => {
                        return Err(ErrorResponse::new("Invalid access token", "INVALID_TOKEN"));
                    }
                    Err(e) => {
                        return Err(ErrorResponse::new(
                            format!("Authentication failed: {}", e),
                            "AUTH_FAILED",
                        ));
                    }
                }
            }
        }
    }

    // Try API key authentication
    if let Some(api_key_header) = headers.get("x-api-key") {
        if let Ok(api_key) = api_key_header.to_str() {
            match state.api_key_service.validate_key(api_key).await {
                Ok(user_id) => {
                    request.extensions_mut().insert(AuthenticatedUser {
                        user_id: user_id.clone(),
                        auth_method: AuthMethod::ApiKey,
                    });

                    // Apply rate limiting
                    return apply_rate_limiting(state, user_id, &path, request, next).await;
                }
                Err(AuthError::KeyRevoked) => {
                    return Err(ErrorResponse::new("API key has been revoked", "KEY_REVOKED"));
                }
                Err(AuthError::InvalidKey) | Err(AuthError::InvalidKeyFormat) => {
                    return Err(ErrorResponse::new("Invalid API key", "INVALID_KEY"));
                }
                Err(e) => {
                    return Err(ErrorResponse::new(
                        format!("API key validation failed: {}", e),
                        "AUTH_FAILED",
                    ));
                }
            }
        }
    }

    // No valid authentication found
    Err(ErrorResponse::new(
        "Authentication required. Provide either Authorization: Bearer <token> or X-API-Key: <key>",
        "UNAUTHORIZED",
    ))
}

/// Apply rate limiting based on user ID and endpoint
async fn apply_rate_limiting(
    state: Arc<ApiState>,
    user_id: UserId,
    endpoint: &str,
    request: Request,
    next: Next,
) -> Result<Response, ErrorResponse> {
    // Create rate limiter
    let limiter = RateLimiter::new(state.rate_limit_store.clone(), state.rate_limit_config.clone());

    // Check rate limit
    let result = limiter
        .check_with_headers(user_id.as_str(), Some(endpoint))
        .await
        .map_err(|e| ErrorResponse::new(format!("Rate limit error: {}", e), "INTERNAL_ERROR"))?;

    if !result.allowed {
        // Rate limit exceeded
        let retry_after = result
            .headers
            .get("Retry-After")
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(60);

        let error = ErrorResponse::new(
            format!("Rate limit exceeded. Retry after {} seconds", retry_after),
            "RATE_LIMIT_EXCEEDED",
        );

        // Build response with rate limit headers
        let mut response = error.into_response();
        let headers = response.headers_mut();
        for (key, value) in result.headers {
            if let Ok(header_name) = key.parse::<axum::http::HeaderName>() {
                if let Ok(header_value) = value.parse::<axum::http::HeaderValue>() {
                    headers.insert(header_name, header_value);
                }
            }
        }

        return Ok(response);
    }

    // Rate limit OK, proceed with request
    let mut response = next.run(request).await;

    // Add rate limit headers to response
    let headers = response.headers_mut();
    for (key, value) in result.headers {
        if let Ok(header_name) = key.parse::<axum::http::HeaderName>() {
            if let Ok(header_value) = value.parse::<axum::http::HeaderValue>() {
                headers.insert(header_name, header_value);
            }
        }
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authenticated_user_creation() {
        let user = AuthenticatedUser {
            user_id: UserId::new("user_123"),
            auth_method: AuthMethod::JwtToken,
        };

        assert_eq!(user.user_id.as_str(), "user_123");
        assert!(matches!(user.auth_method, AuthMethod::JwtToken));
    }
}
