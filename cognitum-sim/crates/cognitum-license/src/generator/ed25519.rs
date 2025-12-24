//! Ed25519-based license generation

use crate::{License, LicenseError, LicenseGenerator, LicenseRequest};
use crate::generator::{generate_license_key, calculate_expiration};
use crate::store::LicenseStore;
use crate::features::FeatureChecker;
use ed25519_dalek::{Signature, Signer, SigningKey};
use chrono::Utc;
use std::sync::Arc;

/// Ed25519-based license generator
pub struct Ed25519Generator {
    /// Private key for signing
    signing_key: SigningKey,

    /// License store for persistence
    store: Arc<dyn LicenseStore>,

    /// Feature checker
    feature_checker: FeatureChecker,
}

impl Ed25519Generator {
    /// Create a new generator with a signing key
    pub fn new(signing_key: SigningKey, store: Arc<dyn LicenseStore>) -> Self {
        Self {
            signing_key,
            store,
            feature_checker: FeatureChecker::new(),
        }
    }

    /// Create generator with test keys for development
    #[cfg(test)]
    pub fn new_with_test_keys() -> Self {
        use crate::store::InMemoryStore;
        let signing_key = SigningKey::from_bytes(&[1u8; 32]);
        Self::new(signing_key, Arc::new(InMemoryStore::new()))
    }

    /// Sign license bytes and return signature
    fn sign_bytes(&self, data: &[u8]) -> Signature {
        self.signing_key.sign(data)
    }
}

impl LicenseGenerator for Ed25519Generator {
    fn generate(&self, request: LicenseRequest) -> Result<License, LicenseError> {
        // Generate unique key
        let key = generate_license_key(request.tier);

        // Calculate expiration
        let valid_until = calculate_expiration(request.duration_months);
        let issued_at = Utc::now();

        // Get tier defaults
        let max_tiles = request.tier.max_tiles();
        let max_simulations_per_month = request.tier.max_simulations_per_month();

        // Combine tier features with custom features
        let mut features = self.feature_checker.features_for_tier(request.tier);
        for custom_feature in request.features {
            if !features.contains(&custom_feature) {
                features.push(custom_feature);
            }
        }

        // Create unsigned license
        let mut license = License {
            key: key.clone(),
            tier: request.tier,
            organization: request.organization,
            email: request.email,
            max_tiles,
            max_simulations_per_month,
            features,
            valid_until,
            issued_at,
            signature: vec![],
            metadata: request.metadata,
        };

        // Sign the license
        self.sign(&mut license)?;

        // Store the license
        self.store.save(&license)
            .map_err(|e| LicenseError::Io(e.to_string()))?;

        Ok(license)
    }

    fn sign(&self, license: &mut License) -> Result<(), LicenseError> {
        let message = license.signable_bytes();
        let signature = self.sign_bytes(&message);
        license.signature = signature.to_bytes().to_vec();
        Ok(())
    }

    fn revoke(&self, key: &str) -> Result<(), LicenseError> {
        // In a real implementation, this would add to a revocation list
        // For now, just remove from store
        self.store.delete(key)
            .map_err(|_| LicenseError::NotFound { key: key.to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::LicenseGenerator;
    use crate::license::LicenseTier;
    use crate::store::InMemoryStore;
    use ed25519_dalek::SigningKey;

    fn create_test_generator() -> Ed25519Generator {
        let signing_key = SigningKey::from_bytes(&[42u8; 32]);
        let store = Arc::new(InMemoryStore::new());
        Ed25519Generator::new(signing_key, store)
    }

    #[test]
    fn test_generate_license() {
        let generator = create_test_generator();

        let request = LicenseRequest {
            tier: LicenseTier::Developer,
            organization: "Test Org".to_string(),
            email: "test@example.com".to_string(),
            duration_months: 12,
            ..Default::default()
        };

        let license = generator.generate(request).unwrap();

        assert!(license.key.starts_with("lic_dev_"));
        assert_eq!(license.tier, LicenseTier::Developer);
        assert_eq!(license.organization, "Test Org");
        assert_eq!(license.max_tiles, 256);
        assert!(!license.signature.is_empty());
        assert_eq!(license.signature.len(), 64);
    }

    #[test]
    fn test_generate_unique_keys() {
        let generator = create_test_generator();

        let request = LicenseRequest {
            tier: LicenseTier::Developer,
            organization: "Test".to_string(),
            email: "test@example.com".to_string(),
            duration_months: 12,
            ..Default::default()
        };

        let license1 = generator.generate(request.clone()).unwrap();
        let license2 = generator.generate(request).unwrap();

        assert_ne!(license1.key, license2.key);
    }

    #[test]
    fn test_sign_license() {
        let generator = create_test_generator();

        let mut license = License {
            key: "lic_dev_test123_abc".to_string(),
            tier: LicenseTier::Developer,
            organization: "Test".to_string(),
            email: "test@example.com".to_string(),
            max_tiles: 256,
            max_simulations_per_month: None,
            features: vec![],
            valid_until: Utc::now() + chrono::Duration::days(365),
            issued_at: Utc::now(),
            signature: vec![],
            metadata: Default::default(),
        };

        generator.sign(&mut license).unwrap();

        assert_eq!(license.signature.len(), 64);
    }

    #[test]
    fn test_tier_limits_applied() {
        let generator = create_test_generator();

        let free_req = LicenseRequest {
            tier: LicenseTier::Free,
            organization: "Test".to_string(),
            email: "test@example.com".to_string(),
            duration_months: 12,
            ..Default::default()
        };

        let free_license = generator.generate(free_req).unwrap();
        assert_eq!(free_license.max_tiles, 32);
        assert_eq!(free_license.max_simulations_per_month, Some(1000));

        let ent_req = LicenseRequest {
            tier: LicenseTier::Enterprise,
            organization: "Test".to_string(),
            email: "test@example.com".to_string(),
            duration_months: 12,
            ..Default::default()
        };

        let ent_license = generator.generate(ent_req).unwrap();
        assert_eq!(ent_license.max_tiles, u32::MAX);
        assert_eq!(ent_license.max_simulations_per_month, None);
    }
}
