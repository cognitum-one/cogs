//! Rate limiting middleware

use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpResponse,
};
use futures::future::{ready, LocalBoxFuture, Ready};
use std::sync::Arc;
use std::task::{Context, Poll};

use crate::models::error::{ApiError, RateLimitError};
use crate::services::{RateLimiter, RateLimitResult};

pub struct RateLimitMiddleware {
    rate_limiter: Arc<dyn RateLimiter>,
}

impl RateLimitMiddleware {
    pub fn new(rate_limiter: Arc<dyn RateLimiter>) -> Self {
        Self { rate_limiter }
    }
}

impl<S, B> Transform<S, ServiceRequest> for RateLimitMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RateLimitMiddlewareService<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimitMiddlewareService {
            service,
            rate_limiter: self.rate_limiter.clone(),
        }))
    }
}

pub struct RateLimitMiddlewareService<S> {
    service: S,
    rate_limiter: Arc<dyn RateLimiter>,
}

impl<S, B> Service<ServiceRequest> for RateLimitMiddlewareService<S>
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
        // Extract user ID from request (set by auth middleware)
        let user_id = req
            .extensions()
            .get::<crate::services::AuthenticatedUser>()
            .map(|u| u.user_id.clone())
            .unwrap_or_else(|| req.peer_addr().map(|a| a.to_string()).unwrap_or_default());

        let rate_limiter = self.rate_limiter.clone();
        let fut = self.service.call(req);

        Box::pin(async move {
            match rate_limiter.check(&user_id).await {
                Ok(RateLimitResult::Allowed { remaining }) => {
                    let mut res = fut.await?;

                    // Add rate limit headers
                    let headers = res.response_mut().headers_mut();
                    headers.insert(
                        actix_web::http::header::HeaderName::from_static("x-ratelimit-remaining"),
                        actix_web::http::header::HeaderValue::from(remaining),
                    );

                    rate_limiter.record(&user_id).await.ok();
                    Ok(res)
                }
                Ok(RateLimitResult::Exceeded { retry_after }) => {
                    let error = ApiError::TooManyRequests {
                        retry_after: retry_after.as_secs(),
                    };
                    Err(error.into())
                }
                Err(e) => {
                    let error: ApiError = e.into();
                    Err(error.into())
                }
            }
        })
    }
}
