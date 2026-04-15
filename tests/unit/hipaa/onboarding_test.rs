//! Unit tests for BAA workflow

use cognitum::hipaa::*;

fn create_test_customer(id: &str, tier: Tier, baa_signed: bool) -> Customer {
    Customer {
        id: CustomerId::new(id),
        tier,
        baa_signed,
        baa_signed_at: if baa_signed {
            Some(chrono::Utc::now())
        } else {
            None
        },
    }
}

#[tokio::test]
async fn enterprise_without_baa_denied_phi_access() {
    let onboarding = CustomerOnboarding::new();
    let customer = create_test_customer("cust_123", Tier::Enterprise, false);

    let result = onboarding.enable_phi_features(&customer).await;
    assert!(matches!(result, Err(HipaaError::BaaRequired)));
}

#[tokio::test]
async fn free_tier_denied_phi_access() {
    let onboarding = CustomerOnboarding::new();
    let customer = create_test_customer("cust_123", Tier::Free, true);

    let result = onboarding.enable_phi_features(&customer).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn professional_tier_denied_phi_access() {
    let onboarding = CustomerOnboarding::new();
    let customer = create_test_customer("cust_123", Tier::Professional, true);

    let result = onboarding.enable_phi_features(&customer).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn enterprise_with_baa_allowed_phi_access() {
    let onboarding = CustomerOnboarding::new();
    let customer_id = CustomerId::new("cust_123");
    let customer = create_test_customer("cust_123", Tier::Enterprise, false);

    // Register customer
    onboarding.register_customer(customer).unwrap();

    // Sign BAA
    let signature = onboarding
        .sign_baa_for_customer(&customer_id, "John Doe".to_string(), "hash123".to_string())
        .unwrap();

    assert_eq!(signature.customer_id, customer_id);
    assert_eq!(signature.signed_by, "John Doe");
    assert_eq!(signature.version, "1.0");

    // Get updated customer
    let updated_customer = onboarding.get_customer(&customer_id).unwrap();
    assert!(updated_customer.baa_signed);
    assert!(updated_customer.baa_signed_at.is_some());

    // Enable PHI features should now succeed
    let result = onboarding.enable_phi_features(&updated_customer).await;
    assert!(result.is_ok());
}

#[test]
fn baa_signature_recorded() {
    let onboarding = CustomerOnboarding::new();
    let customer_id = CustomerId::new("cust_123");
    let customer = create_test_customer("cust_123", Tier::Enterprise, false);

    onboarding.register_customer(customer).unwrap();

    let signature = onboarding
        .sign_baa_for_customer(&customer_id, "Jane Smith".to_string(), "hash456".to_string())
        .unwrap();

    assert_eq!(signature.signed_by, "Jane Smith");
    assert_eq!(signature.document_hash, "hash456");
    assert!(onboarding.has_signed_baa(&customer_id).unwrap());
}

#[test]
fn baa_verification() {
    let onboarding = CustomerOnboarding::new();
    let customer_id = CustomerId::new("cust_123");
    let customer = create_test_customer("cust_123", Tier::Enterprise, false);

    onboarding.register_customer(customer).unwrap();

    // Before BAA
    assert!(!onboarding.has_signed_baa(&customer_id).unwrap());
    assert!(!onboarding.verify_baa(&customer_id).unwrap());

    // Sign BAA
    onboarding
        .sign_baa_for_customer(&customer_id, "Signer".to_string(), "hash".to_string())
        .unwrap();

    // After BAA
    assert!(onboarding.has_signed_baa(&customer_id).unwrap());
    assert!(onboarding.verify_baa(&customer_id).unwrap());
}

#[test]
fn duplicate_baa_signature_prevented() {
    let onboarding = CustomerOnboarding::new();
    let customer_id = CustomerId::new("cust_123");
    let customer = create_test_customer("cust_123", Tier::Enterprise, false);

    onboarding.register_customer(customer).unwrap();

    // First signature succeeds
    let result1 = onboarding.sign_baa_for_customer(
        &customer_id,
        "Person 1".to_string(),
        "hash1".to_string(),
    );
    assert!(result1.is_ok());

    // Second signature fails
    let result2 = onboarding.sign_baa_for_customer(
        &customer_id,
        "Person 2".to_string(),
        "hash2".to_string(),
    );
    assert!(result2.is_err());
}

#[test]
fn customer_registration_and_retrieval() {
    let onboarding = CustomerOnboarding::new();
    let customer = create_test_customer("cust_123", Tier::Enterprise, false);

    onboarding
        .register_customer(customer.clone())
        .unwrap();

    let retrieved = onboarding.get_customer(&customer.id).unwrap();
    assert_eq!(retrieved.id, customer.id);
    assert_eq!(retrieved.tier, customer.tier);
}

#[test]
fn nonexistent_customer_retrieval_fails() {
    let onboarding = CustomerOnboarding::new();
    let customer_id = CustomerId::new("nonexistent");

    let result = onboarding.get_customer(&customer_id);
    assert!(result.is_err());
}
