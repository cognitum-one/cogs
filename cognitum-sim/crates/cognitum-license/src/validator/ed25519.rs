//! Ed25519 cryptographic license validation

use crate::{License, LicenseError, LicenseValidator, Operation, Feature};
use crate::validator::BaseValidator;
use crate::store::LicenseStore;
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
#[cfg(test)]
use ed25519_dalek::SigningKey;
use std::sync::Arc;
use parking_lot::RwLock;
use std::collections::HashSet;

/// Ed25519-based license validator
pub struct Ed25519Validator {
    /// Public key for signature verification
    public_key: VerifyingKey,

    /// License store for persistence
    pub(crate) store: Arc<dyn LicenseStore>,

    /// Base validator for common logic
    base_validator: BaseValidator,

    /// Revoked license keys
    revoked_keys: Arc<RwLock<HashSet<String>>>,
}

impl Ed25519Validator {
    /// Create a new validator with a public key
    pub fn new(public_key: VerifyingKey, store: Arc<dyn LicenseStore>) -> Self {
        Self {
            public_key,
            store,
            base_validator: BaseValidator::new(),
            revoked_keys: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Create validator with test keys for development
    #[cfg(test)]
    pub fn new_with_test_keys() -> Self {
        use crate::store::InMemoryStore;
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        let public_key = signing_key.verifying_key();
        Self::new(public_key, Arc::new(InMemoryStore::new()))
    }

    /// Verify Ed25519 signature on license
    pub fn verify_signature(&self, license: &License) -> Result<(), LicenseError> {
        if license.signature.len() != 64 {
            return Err(LicenseError::InvalidSignature);
        }

        let signature = Signature::from_bytes(
            license.signature.as_slice().try_into()
                .map_err(|_| LicenseError::InvalidSignature)?
        );

        let message = license.signable_bytes();

        self.public_key
            .verify(&message, &signature)
            .map_err(|_| LicenseError::InvalidSignature)?;

        Ok(())
    }

    /// Check if license is revoked
    pub fn is_revoked(&self, key: &str) -> bool {
        self.revoked_keys.read().contains(key)
    }

    /// Revoke a license key
    pub fn revoke(&self, key: &str) {
        self.revoked_keys.write().insert(key.to_string());
    }

    /// Validate license from store
    fn validate_from_store(&self, key: &str) -> Result<License, LicenseError> {
        self.store.get(key)
            .map_err(|_| LicenseError::NotFound { key: key.to_string() })
    }
}

impl LicenseValidator for Ed25519Validator {
    fn validate(&self, key: &str) -> Result<License, LicenseError> {
        // Parse key format
        self.base_validator.parse_key(key)?;

        // Check if revoked
        if self.is_revoked(key) {
            return Err(LicenseError::Revoked);
        }

        // Get license from store
        let license = self.validate_from_store(key)?;

        // Verify signature
        self.verify_signature(&license)?;

        // Check expiration
        self.base_validator.check_expiration(&license)?;

        Ok(license)
    }

    fn check_feature(&self, license: &License, feature: Feature) -> bool {
        self.base_validator.check_feature(license, feature)
    }

    fn check_limits(&self, license: &License, operation: Operation) -> Result<(), LicenseError> {
        self.base_validator.check_limits(license, operation)
    }

    fn refresh(&self, license: &License) -> Result<License, LicenseError> {
        // In a real implementation, this would contact a license server
        // For now, just re-validate from store
        self.validate(&license.key)
    }

    fn validate_offline(&self, license: &License) -> Result<(), LicenseError> {
        // Verify signature
        self.verify_signature(license)?;

        // Check expiration
        self.base_validator.check_expiration(license)?;

        // Note: Cannot check revocation status offline
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::{Ed25519Generator, LicenseGenerator};
    use crate::license::{LicenseRequest, LicenseTier};
    use crate::store::InMemoryStore;

    fn create_test_validator_and_generator() -> (Ed25519Validator, Ed25519Generator) {
        let signing_key = SigningKey::from_bytes(&[42u8; 32]);
        let public_key = signing_key.verifying_key();

        let store = Arc::new(InMemoryStore::new());
        let validator = Ed25519Validator::new(public_key, store.clone());
        let generator = Ed25519Generator::new(signing_key, store);

        (validator, generator)
    }

    #[test]
    fn test_verify_valid_signature() {
        let (validator, generator) = create_test_validator_and_generator();

        let request = LicenseRequest {
            tier: LicenseTier::Developer,
            organization: "Test Org".to_string(),
            email: "test@example.com".to_string(),
            duration_months: 12,
            ..Default::default()
        };

        let license = generator.generate(request).unwrap();
        let result = validator.verify_signature(&license);

        assert!(result.is_ok());
    }

    #[test]
    fn test_reject_invalid_signature() {
        let (validator, generator) = create_test_validator_and_generator();

        let request = LicenseRequest {
            tier: LicenseTier::Developer,
            organization: "Test Org".to_string(),
            email: "test@example.com".to_string(),
            duration_months: 12,
            ..Default::default()
        };

        let mut license = generator.generate(request).unwrap();

        // Corrupt signature
        license.signature[0] ^= 0xFF;

        let result = validator.verify_signature(&license);
        assert!(matches!(result, Err(LicenseError::InvalidSignature)));
    }

    #[test]
    fn test_revocation() {
        let (validator, generator) = create_test_validator_and_generator();

        let request = LicenseRequest {
            tier: LicenseTier::Developer,
            organization: "Test Org".to_string(),
            email: "test@example.com".to_string(),
            duration_months: 12,
            ..Default::default()
        };

        let license = generator.generate(request).unwrap();

        // Should validate initially
        assert!(validator.validate(&license.key).is_ok());

        // Revoke
        validator.revoke(&license.key);

        // Should now fail
        assert!(matches!(
            validator.validate(&license.key),
            Err(LicenseError::Revoked)
        ));
    }

    #[test]
    fn test_validate_expired_license() {
        let (validator, _) = create_test_validator_and_generator();

        let license = License {
            key: "lic_dev_test123_abc".to_string(),
            tier: LicenseTier::Developer,
            organization: "Test".to_string(),
            email: "test@example.com".to_string(),
            max_tiles: 256,
            max_simulations_per_month: None,
            features: vec![],
            valid_until: chrono::Utc::now() - chrono::Duration::days(1),
            issued_at: chrono::Utc::now() - chrono::Duration::days(365),
            signature: vec![0; 64],
            metadata: Default::default(),
        };

        let result = validator.base_validator.check_expiration(&license);
        assert!(matches!(result, Err(LicenseError::Expired { .. })));
    }
}
