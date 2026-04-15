//! Acceptance tests for license validation

use cognitum_license::validator::{LicenseValidator, Ed25519Validator};
use cognitum_license::generator::{LicenseGenerator, Ed25519Generator};
use cognitum_license::{LicenseRequest, LicenseTier, LicenseError};
use cognitum_license::store::InMemoryStore;
use ed25519_dalek::SigningKey;
use std::sync::Arc;
use chrono::Utc;

fn create_test_validator_and_generator() -> (Ed25519Validator, Ed25519Generator) {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let public_key = signing_key.verifying_key();

    let store = Arc::new(InMemoryStore::new());
    let validator = Ed25519Validator::new(public_key, store.clone());
    let generator = Ed25519Generator::new(signing_key, store);

    (validator, generator)
}

#[test]
fn should_accept_valid_license_key() {
    // Given: A valid license key (generated for tests)
    let (validator, generator) = create_test_validator_and_generator();

    let request = LicenseRequest {
        tier: LicenseTier::Developer,
        organization: "Test Org".to_string(),
        email: "test@example.com".to_string(),
        duration_months: 12,
        ..Default::default()
    };

    let license = generator.generate(request).unwrap();

    // When: Validating the key
    let result = validator.validate(&license.key);

    // Then: Returns valid license
    assert!(result.is_ok());
    let validated = result.unwrap();
    assert_eq!(validated.tier, LicenseTier::Developer);
    assert!(validated.valid_until > Utc::now());
}

#[test]
fn should_reject_invalid_license_key() {
    let (validator, _) = create_test_validator_and_generator();

    let result = validator.validate("invalid_key");

    assert!(matches!(result, Err(LicenseError::InvalidKey)));
}

#[test]
fn should_reject_expired_license() {
    use cognitum_license::validator::BaseValidator;

    // Create a license that is already expired
    let license = cognitum_license::License {
        key: "lic_dev_test123_abc".to_string(),
        tier: LicenseTier::Developer,
        organization: "Test".to_string(),
        email: "test@example.com".to_string(),
        max_tiles: 256,
        max_simulations_per_month: None,
        features: vec![],
        valid_until: Utc::now() - chrono::Duration::days(1),
        issued_at: Utc::now() - chrono::Duration::days(365),
        signature: vec![0; 64],
        metadata: Default::default(),
    };

    // Test that the validator detects expiration
    let base_validator = BaseValidator::new();
    let result = base_validator.check_expiration(&license);

    assert!(matches!(result, Err(LicenseError::Expired { .. })));
}

#[test]
fn should_verify_cryptographic_signature() {
    let (validator, generator) = create_test_validator_and_generator();

    let mut license = generator.generate(LicenseRequest {
        tier: LicenseTier::Developer,
        organization: "Test".to_string(),
        email: "test@example.com".to_string(),
        duration_months: 12,
        ..Default::default()
    }).unwrap();

    // Tamper with signature
    license.signature[0] ^= 0xFF;

    // Test signature verification directly
    let result = validator.verify_signature(&license);

    assert!(matches!(result, Err(LicenseError::InvalidSignature)));
}

#[test]
fn should_validate_offline_with_cached_license() {
    let (validator, generator) = create_test_validator_and_generator();

    let license = generator.generate(LicenseRequest {
        tier: LicenseTier::Developer,
        organization: "Test".to_string(),
        email: "test@example.com".to_string(),
        duration_months: 12,
        ..Default::default()
    }).unwrap();

    let result = validator.validate_offline(&license);

    assert!(result.is_ok());
}

#[test]
fn should_handle_license_revocation() {
    let (validator, generator) = create_test_validator_and_generator();

    let license = generator.generate(LicenseRequest {
        tier: LicenseTier::Developer,
        organization: "Test".to_string(),
        email: "test@example.com".to_string(),
        duration_months: 12,
        ..Default::default()
    }).unwrap();

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
