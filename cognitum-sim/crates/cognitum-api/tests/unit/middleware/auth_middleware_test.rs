//! Unit tests for authentication middleware

#[cfg(test)]
mod tests {
    use cognitum_api::services::{MockAuthService, AuthenticatedUser, UserTier};
    use cognitum_api::models::error::AuthError;

    #[tokio::test]
    async fn should_validate_api_key() {
        let mut mock_auth = MockAuthService::new();
        mock_auth
            .expect_validate_api_key()
            .with(mockall::predicate::eq("sk_test_123"))
            .times(1)
            .returning(|_| {
                Ok(AuthenticatedUser {
                    user_id: "test_user".to_string(),
                    api_key: "sk_test_123".to_string(),
                    tier: UserTier::Free,
                })
            });

        let result = mock_auth.validate_api_key("sk_test_123").await;

        assert!(result.is_ok());
        let user = result.unwrap();
        assert_eq!(user.user_id, "test_user");
    }

    #[tokio::test]
    async fn should_reject_invalid_api_key() {
        let mut mock_auth = MockAuthService::new();
        mock_auth
            .expect_validate_api_key()
            .returning(|_| Err(AuthError::InvalidApiKey));

        let result = mock_auth.validate_api_key("sk_invalid").await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AuthError::InvalidApiKey));
    }
}
