//! Unit tests for license validation

use cognitum_license::validator::{Ed25519Validator, BaseValidator};
use cognitum_license::{License, LicenseError, LicenseTier, LicenseGenerator, Ed25519Generator};
use cognitum_license::store::InMemoryStore;
use ed25519_dalek::SigningKey;
use std::sync::Arc;

fn create_test_validator_and_generator() -> (Ed25519Validator, Ed25519Generator) {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let public_key = signing_key.verifying_key();

    let store = Arc::new(InMemoryStore::new());
    let validator = Ed25519Validator::new(public_key, store.clone());
    let generator = Ed25519Generator::new(signing_key, store);

    (validator, generator)
}

#[test]
fn should_verify_ed25519_signature() {
    let (validator, generator) = create_test_validator_and_generator();

    let request = cognitum_license::LicenseRequest {
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
fn should_reject_invalid_signature() {
    let (validator, generator) = create_test_validator_and_generator();

    let request = cognitum_license::LicenseRequest {
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
fn should_parse_license_key_format() {
    let validator = BaseValidator::new();

    // Valid format: lic_{tier}_{random}_{checksum}
    assert!(validator.parse_key("lic_dev_abc123_xyz").is_ok());
    assert!(validator.parse_key("lic_pro_def456_uvw").is_ok());

    // Invalid formats
    assert!(validator.parse_key("invalid").is_err());
    assert!(validator.parse_key("lic_").is_err());
    assert!(validator.parse_key("lic_unknown_abc_xyz").is_err());
}

#[test]
fn should_check_expiration() {
    use chrono::{Utc, Duration};

    let validator = BaseValidator::new();

    let valid_license = License {
        key: "test_key".to_string(),
        tier: LicenseTier::Free,
        organization: "Test".to_string(),
        email: "test@example.com".to_string(),
        max_tiles: 32,
        max_simulations_per_month: Some(1000),
        features: vec![],
        valid_until: Utc::now() + Duration::days(30),
        issued_at: Utc::now(),
        signature: vec![],
        metadata: Default::default(),
    };

    let expired_license = License {
        valid_until: Utc::now() - Duration::days(1),
        ..valid_license.clone()
    };

    assert!(validator.check_expiration(&valid_license).is_ok());
    assert!(matches!(
        validator.check_expiration(&expired_license),
        Err(LicenseError::Expired { .. })
    ));
}
