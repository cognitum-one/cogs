//! License validation unit tests

use mockall::mock;
use mockall::predicate::*;

#[cfg(test)]
mod validation_tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    mock! {
        pub LicenseStore {
            fn get_license(&self, key: &str) -> Result<Option<License>, LicenseError>;
            fn revoke_license(&self, key: &str) -> Result<(), LicenseError>;
        }
    }

    #[derive(Debug, Clone)]
    pub struct License {
        pub key: String,
        pub tier: String,
        pub expires_at: SystemTime,
        pub revoked: bool,
    }

    #[derive(Debug, thiserror::Error)]
    pub enum LicenseError {
        #[error("License not found")]
        NotFound,
        #[error("License expired")]
        Expired,
        #[error("License revoked")]
        Revoked,
    }

    pub struct LicenseValidator;

    impl LicenseValidator {
        pub fn validate<S: LicenseStore>(
            store: &S,
            key: &str,
        ) -> Result<License, LicenseError> {
            let license = store
                .get_license(key)?
                .ok_or(LicenseError::NotFound)?;

            if license.revoked {
                return Err(LicenseError::Revoked);
            }

            if license.expires_at < SystemTime::now() {
                return Err(LicenseError::Expired);
            }

            Ok(license)
        }
    }

    #[test]
    fn should_validate_active_license() {
        // Given: A valid license
        let mut mock_store = MockLicenseStore::new();

        let future_time = SystemTime::now() + Duration::from_secs(86400);
        let license = License {
            key: "lic_valid_123".to_string(),
            tier: "professional".to_string(),
            expires_at: future_time,
            revoked: false,
        };

        mock_store
            .expect_get_license()
            .with(eq("lic_valid_123"))
            .returning(move |_| Ok(Some(license.clone())));

        // When: Validating
        let result = LicenseValidator::validate(&mock_store, "lic_valid_123");

        // Then: Should succeed
        assert!(result.is_ok());
    }

    #[test]
    fn should_reject_expired_license() {
        // Given: An expired license
        let mut mock_store = MockLicenseStore::new();

        let past_time = SystemTime::now() - Duration::from_secs(86400);
        let license = License {
            key: "lic_expired".to_string(),
            tier: "professional".to_string(),
            expires_at: past_time,
            revoked: false,
        };

        mock_store
            .expect_get_license()
            .returning(move |_| Ok(Some(license.clone())));

        // When: Validating
        let result = LicenseValidator::validate(&mock_store, "lic_expired");

        // Then: Should fail
        assert!(matches!(result, Err(LicenseError::Expired)));
    }

    #[test]
    fn should_reject_revoked_license() {
        // Given: A revoked license
        let mut mock_store = MockLicenseStore::new();

        let license = License {
            key: "lic_revoked".to_string(),
            tier: "professional".to_string(),
            expires_at: SystemTime::now() + Duration::from_secs(86400),
            revoked: true,
        };

        mock_store
            .expect_get_license()
            .returning(move |_| Ok(Some(license.clone())));

        // When: Validating
        let result = LicenseValidator::validate(&mock_store, "lic_revoked");

        // Then: Should fail
        assert!(matches!(result, Err(LicenseError::Revoked)));
    }

    #[test]
    fn should_reject_nonexistent_license() {
        // Given: Store without license
        let mut mock_store = MockLicenseStore::new();

        mock_store
            .expect_get_license()
            .returning(|_| Ok(None));

        // When: Validating
        let result = LicenseValidator::validate(&mock_store, "lic_notfound");

        // Then: Should fail
        assert!(matches!(result, Err(LicenseError::NotFound)));
    }
}
