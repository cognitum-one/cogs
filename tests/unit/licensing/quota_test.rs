//! Quota enforcement unit tests

use mockall::mock;
use mockall::predicate::*;

#[cfg(test)]
mod quota_tests {
    use super::*;

    mock! {
        pub QuotaStore {
            fn get_usage(&self, user_id: &str, resource: &str) -> Result<u32, QuotaError>;
            fn increment_usage(&self, user_id: &str, resource: &str) -> Result<u32, QuotaError>;
            fn reset_usage(&self, user_id: &str, resource: &str) -> Result<(), QuotaError>;
        }
    }

    #[derive(Debug, thiserror::Error)]
    pub enum QuotaError {
        #[error("Quota exceeded")]
        Exceeded,
        #[error("Store error")]
        StoreError,
    }

    pub struct QuotaEnforcer {
        limit: u32,
    }

    impl QuotaEnforcer {
        pub fn new(limit: u32) -> Self {
            Self { limit }
        }

        pub fn check_and_increment<S: QuotaStore>(
            &self,
            store: &S,
            user_id: &str,
            resource: &str,
        ) -> Result<u32, QuotaError> {
            let usage = store.get_usage(user_id, resource)?;

            if usage >= self.limit {
                return Err(QuotaError::Exceeded);
            }

            store.increment_usage(user_id, resource)
        }
    }

    #[test]
    fn should_allow_usage_under_limit() {
        // Given: A store with usage below limit
        let mut mock_store = MockQuotaStore::new();

        mock_store
            .expect_get_usage()
            .with(eq("user_123"), eq("simulations"))
            .returning(|_, _| Ok(50));

        mock_store
            .expect_increment_usage()
            .with(eq("user_123"), eq("simulations"))
            .returning(|_, _| Ok(51));

        let enforcer = QuotaEnforcer::new(100);

        // When: Checking quota
        let result = enforcer.check_and_increment(&mock_store, "user_123", "simulations");

        // Then: Should succeed
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 51);
    }

    #[test]
    fn should_reject_usage_at_limit() {
        // Given: A store at quota limit
        let mut mock_store = MockQuotaStore::new();

        mock_store
            .expect_get_usage()
            .returning(|_, _| Ok(100));

        let enforcer = QuotaEnforcer::new(100);

        // When: Attempting to exceed quota
        let result = enforcer.check_and_increment(&mock_store, "user_123", "simulations");

        // Then: Should be rejected
        assert!(matches!(result, Err(QuotaError::Exceeded)));
    }

    #[test]
    fn should_reject_usage_over_limit() {
        // Given: A store over quota
        let mut mock_store = MockQuotaStore::new();

        mock_store
            .expect_get_usage()
            .returning(|_, _| Ok(150));

        let enforcer = QuotaEnforcer::new(100);

        // When: Checking quota
        let result = enforcer.check_and_increment(&mock_store, "user_123", "simulations");

        // Then: Should be rejected
        assert!(matches!(result, Err(QuotaError::Exceeded)));
    }

    #[test]
    fn should_reset_quota_successfully() {
        // Given: A store with usage
        let mut mock_store = MockQuotaStore::new();

        mock_store
            .expect_reset_usage()
            .with(eq("user_123"), eq("simulations"))
            .times(1)
            .returning(|_, _| Ok(()));

        // When: Resetting quota
        let result = mock_store.reset_usage("user_123", "simulations");

        // Then: Should succeed
        assert!(result.is_ok());
    }
}
