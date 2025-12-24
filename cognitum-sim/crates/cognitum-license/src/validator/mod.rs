//! License validation and verification

use crate::{License, LicenseError, LicenseTier, Operation, Feature, FeatureChecker};
use mockall::automock;

pub mod ed25519;

pub use ed25519::Ed25519Validator;

/// Trait for license validation
#[automock]
pub trait LicenseValidator: Send + Sync {
    /// Validate license key and return license details
    fn validate(&self, key: &str) -> Result<License, LicenseError>;

    /// Check if specific feature is enabled
    fn check_feature(&self, license: &License, feature: Feature) -> bool;

    /// Check if operation is within limits
    fn check_limits(&self, license: &License, operation: Operation) -> Result<(), LicenseError>;

    /// Refresh license from server (for online validation)
    fn refresh(&self, license: &License) -> Result<License, LicenseError>;

    /// Validate offline with cached license
    fn validate_offline(&self, license: &License) -> Result<(), LicenseError>;
}

/// Common validation logic
pub struct BaseValidator {
    feature_checker: FeatureChecker,
}

impl BaseValidator {
    pub fn new() -> Self {
        Self {
            feature_checker: FeatureChecker::new(),
        }
    }

    /// Check if license is expired
    pub fn check_expiration(&self, license: &License) -> Result<(), LicenseError> {
        if license.is_expired() {
            return Err(LicenseError::Expired {
                expired_at: license.valid_until.to_rfc3339(),
            });
        }
        Ok(())
    }

    /// Parse and validate license key format
    pub fn parse_key(&self, key: &str) -> Result<(LicenseTier, String, String), LicenseError> {
        let parts: Vec<&str> = key.split('_').collect();
        if parts.len() != 4 || parts[0] != "lic" {
            return Err(LicenseError::InvalidKey);
        }

        let tier = LicenseTier::from_code(parts[1])
            .ok_or(LicenseError::InvalidKey)?;

        Ok((tier, parts[2].to_string(), parts[3].to_string()))
    }

    /// Check feature availability
    pub fn check_feature(&self, license: &License, feature: Feature) -> bool {
        self.feature_checker.check(license, feature)
    }

    /// Check operation limits
    pub fn check_limits(&self, license: &License, operation: Operation) -> Result<(), LicenseError> {
        match operation {
            Operation::CreateSimulation { tiles } => {
                let max_tiles = license.max_tiles();
                if tiles > max_tiles {
                    return Err(LicenseError::TileLimitExceeded {
                        max: max_tiles,
                        requested: tiles,
                    });
                }
            }
            Operation::ApiRequest { .. } => {
                // Check if API access is available
                if !self.check_feature(license, Feature::ApiAccess) {
                    return Err(LicenseError::FeatureNotAvailable {
                        feature: "API Access".to_string(),
                        tier: license.tier.to_string(),
                    });
                }
            }
            Operation::RunSimulation => {
                // Additional checks can be done by the meter
            }
        }
        Ok(())
    }
}

impl Default for BaseValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::License;
    use chrono::Utc;

    fn create_test_license(tier: LicenseTier) -> License {
        License {
            key: format!("lic_{}_test123_abc", tier.tier_code()),
            tier,
            organization: "Test Org".to_string(),
            email: "test@example.com".to_string(),
            max_tiles: tier.max_tiles(),
            max_simulations_per_month: tier.max_simulations_per_month(),
            features: vec![],
            valid_until: Utc::now() + chrono::Duration::days(365),
            issued_at: Utc::now(),
            signature: vec![0; 64],
            metadata: Default::default(),
        }
    }

    #[test]
    fn test_parse_valid_key() {
        let validator = BaseValidator::new();
        let result = validator.parse_key("lic_dev_abc123_xyz");
        assert!(result.is_ok());
        let (tier, random, checksum) = result.unwrap();
        assert_eq!(tier, LicenseTier::Developer);
        assert_eq!(random, "abc123");
        assert_eq!(checksum, "xyz");
    }

    #[test]
    fn test_parse_invalid_key() {
        let validator = BaseValidator::new();
        assert!(validator.parse_key("invalid").is_err());
        assert!(validator.parse_key("lic_").is_err());
        assert!(validator.parse_key("lic_unknown_abc_xyz").is_err());
    }

    #[test]
    fn test_check_expiration() {
        let validator = BaseValidator::new();

        let valid_license = License {
            valid_until: Utc::now() + chrono::Duration::days(30),
            ..create_test_license(LicenseTier::Free)
        };
        assert!(validator.check_expiration(&valid_license).is_ok());

        let expired_license = License {
            valid_until: Utc::now() - chrono::Duration::days(1),
            ..create_test_license(LicenseTier::Free)
        };
        assert!(matches!(
            validator.check_expiration(&expired_license),
            Err(LicenseError::Expired { .. })
        ));
    }

    #[test]
    fn test_tile_limit_enforcement() {
        let validator = BaseValidator::new();
        let free_license = create_test_license(LicenseTier::Free);

        // Within limit
        assert!(validator.check_limits(
            &free_license,
            Operation::CreateSimulation { tiles: 32 }
        ).is_ok());

        // Exceeds limit
        assert!(matches!(
            validator.check_limits(
                &free_license,
                Operation::CreateSimulation { tiles: 33 }
            ),
            Err(LicenseError::TileLimitExceeded { .. })
        ));
    }

    #[test]
    fn test_api_access_gating() {
        let validator = BaseValidator::new();

        // Free tier has no API access
        let free_license = create_test_license(LicenseTier::Free);
        assert!(matches!(
            validator.check_limits(
                &free_license,
                Operation::ApiRequest { endpoint: "/test".to_string() }
            ),
            Err(LicenseError::FeatureNotAvailable { .. })
        ));

        // Developer tier has API access
        let dev_license = create_test_license(LicenseTier::Developer);
        assert!(validator.check_limits(
            &dev_license,
            Operation::ApiRequest { endpoint: "/test".to_string() }
        ).is_ok());
    }
}
