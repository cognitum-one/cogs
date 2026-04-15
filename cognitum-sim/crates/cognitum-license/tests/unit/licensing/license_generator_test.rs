//! Unit tests for license generation

use cognitum_license::generator::{Ed25519Generator, generate_license_key, LicenseGenerator};
use cognitum_license::{LicenseRequest, LicenseTier};
use cognitum_license::store::InMemoryStore;
use ed25519_dalek::SigningKey;
use std::sync::Arc;

fn create_test_generator() -> Ed25519Generator {
    let signing_key = SigningKey::from_bytes(&[42u8; 32]);
    let store = Arc::new(InMemoryStore::new());
    Ed25519Generator::new(signing_key, store)
}

#[test]
fn should_generate_unique_keys() {
    let generator = create_test_generator();

    let request = LicenseRequest {
        tier: LicenseTier::Developer,
        organization: "Test".to_string(),
        email: "test@example.com".to_string(),
        duration_months: 12,
        ..Default::default()
    };

    let key1 = generator.generate(request.clone()).unwrap().key;
    let key2 = generator.generate(request).unwrap().key;

    assert_ne!(key1, key2);
}

#[test]
fn should_include_tier_in_key() {
    let dev_key = generate_license_key(LicenseTier::Developer);
    let pro_key = generate_license_key(LicenseTier::Professional);

    assert!(dev_key.contains("dev"));
    assert!(pro_key.contains("pro"));
}

#[test]
fn should_set_expiration_from_duration() {
    use chrono::{Utc, Duration};

    let generator = create_test_generator();

    let license = generator.generate(LicenseRequest {
        tier: LicenseTier::Developer,
        organization: "Test".to_string(),
        email: "test@example.com".to_string(),
        duration_months: 12,
        ..Default::default()
    }).unwrap();

    let expected_expiry = Utc::now() + Duration::days(360);
    let diff = (license.valid_until - expected_expiry).num_days().abs();

    assert!(diff <= 1); // Within 1 day tolerance
}

#[test]
fn should_sign_with_ed25519() {
    let generator = create_test_generator();

    let license = generator.generate(LicenseRequest {
        tier: LicenseTier::Developer,
        organization: "Test".to_string(),
        email: "test@example.com".to_string(),
        duration_months: 12,
        ..Default::default()
    }).unwrap();

    // Verify signature length (Ed25519 = 64 bytes)
    assert_eq!(license.signature.len(), 64);
}

#[test]
fn should_apply_tier_limits() {
    let generator = create_test_generator();

    let free_license = generator.generate(LicenseRequest {
        tier: LicenseTier::Free,
        organization: "Test".to_string(),
        email: "test@example.com".to_string(),
        duration_months: 12,
        ..Default::default()
    }).unwrap();

    assert_eq!(free_license.max_tiles, 32);
    assert_eq!(free_license.max_simulations_per_month, Some(1000));

    let ent_license = generator.generate(LicenseRequest {
        tier: LicenseTier::Enterprise,
        organization: "Test".to_string(),
        email: "test@example.com".to_string(),
        duration_months: 12,
        ..Default::default()
    }).unwrap();

    assert_eq!(ent_license.max_tiles, u32::MAX);
    assert_eq!(ent_license.max_simulations_per_month, None);
}
