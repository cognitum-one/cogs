//! Acceptance tests for feature gating

use cognitum_license::validator::{LicenseValidator, Ed25519Validator};
use cognitum_license::generator::{LicenseGenerator, Ed25519Generator};
use cognitum_license::{LicenseRequest, LicenseTier, Operation};
use cognitum_license::{Feature, LicenseError};
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

fn create_license(_validator: &Ed25519Validator, generator: &Ed25519Generator, tier: LicenseTier) -> cognitum_license::License {
    generator.generate(LicenseRequest {
        tier,
        organization: "Test".to_string(),
        email: "test@example.com".to_string(),
        duration_months: 12,
        ..Default::default()
    }).unwrap()
}

#[test]
fn should_gate_api_access_for_free_tier() {
    let (validator, generator) = create_test_validator_and_generator();
    let license = create_license(&validator, &generator, LicenseTier::Free);

    let has_api = validator.check_feature(&license, Feature::ApiAccess);

    assert!(!has_api);
}

#[test]
fn should_allow_api_access_for_developer_tier() {
    let (validator, generator) = create_test_validator_and_generator();
    let license = create_license(&validator, &generator, LicenseTier::Developer);

    let has_api = validator.check_feature(&license, Feature::ApiAccess);

    assert!(has_api);
}

#[test]
fn should_gate_hipaa_compliance_for_non_enterprise() {
    let (validator, generator) = create_test_validator_and_generator();

    let free = create_license(&validator, &generator, LicenseTier::Free);
    let dev = create_license(&validator, &generator, LicenseTier::Developer);
    let pro = create_license(&validator, &generator, LicenseTier::Professional);
    let ent = create_license(&validator, &generator, LicenseTier::Enterprise);

    assert!(!validator.check_feature(&free, Feature::HipaaCompliance));
    assert!(!validator.check_feature(&dev, Feature::HipaaCompliance));
    assert!(!validator.check_feature(&pro, Feature::HipaaCompliance));
    assert!(validator.check_feature(&ent, Feature::HipaaCompliance));
}

#[test]
fn should_enforce_tile_limits_per_tier() {
    let (validator, generator) = create_test_validator_and_generator();

    let free = create_license(&validator, &generator, LicenseTier::Free);
    let dev = create_license(&validator, &generator, LicenseTier::Developer);

    // Free: max 32 tiles
    assert!(validator.check_limits(&free, Operation::CreateSimulation { tiles: 32 }).is_ok());
    assert!(validator.check_limits(&free, Operation::CreateSimulation { tiles: 33 }).is_err());

    // Developer: max 256 tiles
    assert!(validator.check_limits(&dev, Operation::CreateSimulation { tiles: 256 }).is_ok());
    assert!(validator.check_limits(&dev, Operation::CreateSimulation { tiles: 257 }).is_err());
}

#[test]
fn should_enforce_api_access_by_tier() {
    let (validator, generator) = create_test_validator_and_generator();

    let free = create_license(&validator, &generator, LicenseTier::Free);
    let dev = create_license(&validator, &generator, LicenseTier::Developer);

    // Free tier should not have API access
    let result = validator.check_limits(&free, Operation::ApiRequest {
        endpoint: "/test".to_string()
    });
    assert!(matches!(result, Err(LicenseError::FeatureNotAvailable { .. })));

    // Developer tier should have API access
    let result = validator.check_limits(&dev, Operation::ApiRequest {
        endpoint: "/test".to_string()
    });
    assert!(result.is_ok());
}
