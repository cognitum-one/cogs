//! API middleware unit tests

use mockall::mock;
use mockall::predicate::*;

#[cfg(test)]
mod middleware_tests {
    use super::*;

    mock! {
        pub AuthMiddleware {
            fn authenticate(&self, token: &str) -> Result<UserId, AuthError>;
            fn authorize(&self, user_id: &UserId, resource: &str, action: &str) -> Result<bool, AuthError>;
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    pub struct UserId(pub String);

    #[derive(Debug, thiserror::Error)]
    pub enum AuthError {
        #[error("Invalid token")]
        InvalidToken,
        #[error("Unauthorized")]
        Unauthorized,
    }

    #[test]
    fn should_authenticate_valid_token() {
        // Given: Middleware with valid token
        let mut mock_auth = MockAuthMiddleware::new();

        mock_auth
            .expect_authenticate()
            .with(eq("valid_token_123"))
            .times(1)
            .returning(|_| Ok(UserId("user_456".to_string())));

        // When: Authenticating
        let result = mock_auth.authenticate("valid_token_123");

        // Then: Should return user ID
        assert!(result.is_ok());
        assert_eq!(result.unwrap().0, "user_456");
    }

    #[test]
    fn should_reject_invalid_token() {
        // Given: Middleware that rejects token
        let mut mock_auth = MockAuthMiddleware::new();

        mock_auth
            .expect_authenticate()
            .returning(|_| Err(AuthError::InvalidToken));

        // When: Authenticating with invalid token
        let result = mock_auth.authenticate("invalid_token");

        // Then: Should fail
        assert!(matches!(result, Err(AuthError::InvalidToken)));
    }

    #[test]
    fn should_authorize_permitted_action() {
        // Given: Middleware with permissions
        let mut mock_auth = MockAuthMiddleware::new();

        mock_auth
            .expect_authorize()
            .with(
                eq(UserId("user_123".to_string())),
                eq("simulation"),
                eq("start"),
            )
            .times(1)
            .returning(|_, _, _| Ok(true));

        // When: Checking authorization
        let result = mock_auth.authorize(
            &UserId("user_123".to_string()),
            "simulation",
            "start",
        );

        // Then: Should be authorized
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn should_deny_unpermitted_action() {
        // Given: Middleware without permissions
        let mut mock_auth = MockAuthMiddleware::new();

        mock_auth
            .expect_authorize()
            .returning(|_, _, _| Ok(false));

        // When: Checking unauthorized action
        let result = mock_auth.authorize(
            &UserId("user_123".to_string()),
            "admin",
            "delete",
        );

        // Then: Should be denied
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
