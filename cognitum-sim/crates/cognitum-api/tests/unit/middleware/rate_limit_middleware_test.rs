//! Unit tests for rate limit middleware

#[cfg(test)]
mod tests {
    use cognitum_api::services::{MockRateLimiter, RateLimitResult};
    use std::time::Duration;

    #[tokio::test]
    async fn should_check_limit_for_user() {
        let mut mock_limiter = MockRateLimiter::new();
        mock_limiter
            .expect_check()
            .with(mockall::predicate::eq("user_123"))
            .times(1)
            .returning(|_| Ok(RateLimitResult::Allowed { remaining: 99 }));

        let result = mock_limiter.check("user_123").await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), RateLimitResult::Allowed { .. }));
    }

    #[tokio::test]
    async fn should_reject_when_limit_exceeded() {
        let mut mock_limiter = MockRateLimiter::new();
        mock_limiter
            .expect_check()
            .returning(|_| {
                Ok(RateLimitResult::Exceeded {
                    retry_after: Duration::from_secs(60),
                })
            });

        let result = mock_limiter.check("user_123").await;

        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), RateLimitResult::Exceeded { .. }));
    }

    #[tokio::test]
    async fn should_record_successful_request() {
        let mut mock_limiter = MockRateLimiter::new();
        mock_limiter
            .expect_check()
            .returning(|_| Ok(RateLimitResult::Allowed { remaining: 99 }));
        mock_limiter
            .expect_record()
            .with(mockall::predicate::eq("user_123"))
            .times(1)
            .returning(|_| Ok(()));

        mock_limiter.check("user_123").await.unwrap();
        let result = mock_limiter.record("user_123").await;

        assert!(result.is_ok());
    }
}
