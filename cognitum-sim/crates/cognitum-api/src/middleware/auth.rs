//! Authentication middleware

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use futures::future::{ready, LocalBoxFuture, Ready};
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::models::error::{ApiError, AuthError};
use crate::services::{AuthService, AuthenticatedUser};

pub struct AuthMiddleware {
    auth_service: Arc<dyn AuthService>,
}

impl AuthMiddleware {
    pub fn new(auth_service: Arc<dyn AuthService>) -> Self {
        Self { auth_service }
    }

    /// Extract API key from Authorization header
    fn extract_api_key(req: &ServiceRequest) -> Result<String, AuthError> {
        req.headers()
            .get("Authorization")
            .ok_or(AuthError::MissingApiKey)?
            .to_str()
            .map_err(|_| AuthError::InvalidFormat)?
            .strip_prefix("Bearer ")
            .ok_or(AuthError::InvalidFormat)
            .map(String::from)
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService {
            service,
            auth_service: self.auth_service.clone(),
        }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: S,
    auth_service: Arc<dyn AuthService>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Skip auth for health and metrics endpoints
        let path = req.path();
        if path == "/health" || path == "/metrics" {
            let fut = self.service.call(req);
            return Box::pin(async move { fut.await });
        }

        let api_key = match Self::extract_api_key(&req) {
            Ok(key) => key,
            Err(e) => {
                let error: ApiError = e.into();
                return Box::pin(async move { Err(error.into()) });
            }
        };

        let auth_service = self.auth_service.clone();
        let fut = self.service.call(req);

        Box::pin(async move {
            let user = auth_service
                .validate_api_key(&api_key)
                .await
                .map_err(|e| -> Error { ApiError::from(e).into() })?;

            let mut res = fut.await?;
            res.request().extensions_mut().insert(user);
            Ok(res)
        })
    }
}

impl<S> AuthMiddlewareService<S> {
    fn extract_api_key(req: &ServiceRequest) -> Result<String, AuthError> {
        req.headers()
            .get("Authorization")
            .ok_or(AuthError::MissingApiKey)?
            .to_str()
            .map_err(|_| AuthError::InvalidFormat)?
            .strip_prefix("Bearer ")
            .ok_or(AuthError::InvalidFormat)
            .map(String::from)
    }
}

/// Extension trait to get authenticated user from request
pub trait AuthenticatedUserExt {
    fn authenticated_user(&self) -> Result<AuthenticatedUser, ApiError>;
}

impl AuthenticatedUserExt for actix_web::HttpRequest {
    fn authenticated_user(&self) -> Result<AuthenticatedUser, ApiError> {
        self.extensions()
            .get::<AuthenticatedUser>()
            .cloned()
            .ok_or_else(|| ApiError::Unauthorized("User not authenticated".to_string()))
    }
}
