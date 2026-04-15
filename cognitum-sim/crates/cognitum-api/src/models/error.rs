//! API error types

use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Authentication failed: {0}")]
    Unauthorized(String),

    #[error("Access forbidden: {0}")]
    Forbidden(String),

    #[error("Resource not found: {0}")]
    NotFound(String),

    #[error("Invalid request: {0}")]
    BadRequest(String),

    #[error("Rate limit exceeded")]
    TooManyRequests { retry_after: u64 },

    #[error("Internal server error")]
    Internal(String),

    #[error("Service error: {0}")]
    Service(#[from] ServiceError),

    #[error("Validation error: {0}")]
    Validation(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    #[error("Simulation not found")]
    SimulationNotFound,

    #[error("Program not found")]
    ProgramNotFound,

    #[error("Invalid simulation config: {0}")]
    InvalidConfig(String),

    #[error("Simulation already running")]
    AlreadyRunning,

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Execution error: {0}")]
    Execution(String),
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Missing API key")]
    MissingApiKey,

    #[error("Invalid API key format")]
    InvalidFormat,

    #[error("Invalid API key")]
    InvalidApiKey,

    #[error("Token expired")]
    TokenExpired,
}

#[derive(Debug, thiserror::Error)]
pub enum RateLimitError {
    #[error("Rate limit exceeded")]
    Exceeded,

    #[error("Internal error: {0}")]
    Internal(String),
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        match self {
            ApiError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ApiError::Forbidden(_) => StatusCode::FORBIDDEN,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::BadRequest(_) | ApiError::Validation(_) => StatusCode::BAD_REQUEST,
            ApiError::TooManyRequests { .. } => StatusCode::TOO_MANY_REQUESTS,
            ApiError::Internal(_) | ApiError::Service(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let error_code = match self {
            ApiError::Unauthorized(_) => "unauthorized",
            ApiError::Forbidden(_) => "forbidden",
            ApiError::NotFound(_) => "not_found",
            ApiError::BadRequest(_) => "bad_request",
            ApiError::Validation(_) => "validation_error",
            ApiError::TooManyRequests { .. } => "rate_limit_exceeded",
            ApiError::Internal(_) => "internal_error",
            ApiError::Service(_) => "service_error",
        };

        let mut response = HttpResponse::build(self.status_code()).json(ErrorResponse {
            error: ErrorDetail {
                code: error_code.to_string(),
                message: self.to_string(),
                details: None,
            },
        });

        if let ApiError::TooManyRequests { retry_after } = self {
            response.insert_header(("Retry-After", retry_after.to_string()));
        }

        response
    }
}

impl fmt::Display for ServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl From<AuthError> for ApiError {
    fn from(err: AuthError) -> Self {
        match err {
            AuthError::MissingApiKey | AuthError::InvalidFormat | AuthError::InvalidApiKey => {
                ApiError::Unauthorized(err.to_string())
            }
            AuthError::TokenExpired => ApiError::Unauthorized("Token expired".to_string()),
        }
    }
}

impl From<RateLimitError> for ApiError {
    fn from(err: RateLimitError) -> Self {
        match err {
            RateLimitError::Exceeded => ApiError::TooManyRequests { retry_after: 60 },
            RateLimitError::Internal(msg) => ApiError::Internal(msg),
        }
    }
}
