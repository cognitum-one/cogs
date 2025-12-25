//! Acceptance tests for BAA (Business Associate Agreement) workflow
//!
//! Validates that only enterprise customers with signed BAA can access PHI features

use cognitum::hipaa::*;

fn create_customer(id: &str, tier: Tier, baa_signed: bool) -> Customer {
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
async fn enterprise_customer_without_baa_cannot_access_phi() {
    // Given: Enterprise customer without signed BAA
    let onboarding = CustomerOnboarding::new();
    let customer = create_customer("enterprise_001", Tier::Enterprise, false);

    // When: Attempting to enable PHI features
    let result = onboarding.enable_phi_features(&customer).await;

    // Then: Access should be denied
    assert!(
        matches!(result, Err(HipaaError::BaaRequired)),
        "Enterprise customer without BAA should not access PHI"
    );
}

#[tokio::test]
async fn free_tier_customer_cannot_access_phi_even_with_baa() {
    // Given: Free tier customer
    let onboarding = CustomerOnboarding::new();
    let customer = create_customer("free_001", Tier::Free, true);

    // When: Attempting to enable PHI features
    let result = onboarding.enable_phi_features(&customer).await;

    // Then: Access should be denied (wrong tier)
    assert!(
        result.is_err(),
        "Free tier should not have PHI access regardless of BAA"
    );
}

#[tokio::test]
async fn professional_tier_cannot_access_phi() {
    // Given: Professional tier customer
    let onboarding = CustomerOnboarding::new();
    let customer = create_customer("pro_001", Tier::Professional, true);

    // When: Attempting to enable PHI features
    let result = onboarding.enable_phi_features(&customer).await;

    // Then: Access should be denied
    assert!(
        result.is_err(),
        "Professional tier should not have PHI access"
    );
}

#[tokio::test]
async fn enterprise_customer_with_baa_can_access_phi() {
    // Given: Enterprise customer
    let onboarding = CustomerOnboarding::new();
    let customer_id = CustomerId::new("enterprise_002");
    let customer = create_customer("enterprise_002", Tier::Enterprise, false);

    onboarding.register_customer(customer).unwrap();

    // When: Customer signs BAA
    let signature = onboarding
        .sign_baa_for_customer(
            &customer_id,
            "Jane Doe, CTO".to_string(),
            "sha256:abc123def456".to_string(),
        )
        .unwrap();

    // Then: BAA should be recorded
    assert_eq!(signature.customer_id, customer_id);
    assert_eq!(signature.signed_by, "Jane Doe, CTO");
    assert!(onboarding.has_signed_baa(&customer_id).unwrap());

    // And: PHI features should be enabled
    let updated_customer = onboarding.get_customer(&customer_id).unwrap();
    let result = onboarding.enable_phi_features(&updated_customer).await;
    assert!(result.is_ok(), "Enterprise customer with BAA should access PHI");
}

#[test]
fn baa_signature_should_be_tracked() {
    // Given: Customer onboarding service
    let onboarding = CustomerOnboarding::new();
    let customer_id = CustomerId::new("enterprise_003");
    let customer = create_customer("enterprise_003", Tier::Enterprise, false);

    onboarding.register_customer(customer).unwrap();

    // When: BAA is signed
    let before_signing = chrono::Utc::now();
    let signature = onboarding
        .sign_baa_for_customer(
            &customer_id,
            "John Smith, CEO".to_string(),
            "sha256:xyz789".to_string(),
        )
        .unwrap();

    // Then: Signature metadata should be complete
    assert_eq!(signature.customer_id, customer_id);
    assert_eq!(signature.signed_by, "John Smith, CEO");
    assert_eq!(signature.document_hash, "sha256:xyz789");
    assert_eq!(signature.version, "1.0");
    assert!(signature.signed_at >= before_signing);
    assert!(signature.signed_at <= chrono::Utc::now());

    // And: Customer record should be updated
    let updated_customer = onboarding.get_customer(&customer_id).unwrap();
    assert!(updated_customer.baa_signed);
    assert!(updated_customer.baa_signed_at.is_some());
}

#[test]
fn duplicate_baa_signature_should_be_prevented() {
    // Given: Customer with signed BAA
    let onboarding = CustomerOnboarding::new();
    let customer_id = CustomerId::new("enterprise_004");
    let customer = create_customer("enterprise_004", Tier::Enterprise, false);

    onboarding.register_customer(customer).unwrap();

    // First signature succeeds
    let result1 = onboarding.sign_baa_for_customer(
        &customer_id,
        "First Signer".to_string(),
        "hash1".to_string(),
    );
    assert!(result1.is_ok());

    // When: Attempting to sign BAA again
    let result2 = onboarding.sign_baa_for_customer(
        &customer_id,
        "Second Signer".to_string(),
        "hash2".to_string(),
    );

    // Then: Second signature should be rejected
    assert!(
        result2.is_err(),
        "Duplicate BAA signature should be prevented"
    );
}

#[test]
fn baa_verification_should_confirm_signature() {
    // Given: Customer onboarding service
    let onboarding = CustomerOnboarding::new();
    let customer_id = CustomerId::new("enterprise_005");
    let customer = create_customer("enterprise_005", Tier::Enterprise, false);

    onboarding.register_customer(customer).unwrap();

    // When: Before BAA is signed
    assert!(!onboarding.has_signed_baa(&customer_id).unwrap());
    assert!(!onboarding.verify_baa(&customer_id).unwrap());

    // And: After BAA is signed
    onboarding
        .sign_baa_for_customer(&customer_id, "Signer".to_string(), "hash".to_string())
        .unwrap();

    // Then: Verification should confirm signature
    assert!(onboarding.has_signed_baa(&customer_id).unwrap());
    assert!(onboarding.verify_baa(&customer_id).unwrap());
}

#[tokio::test]
async fn complete_enterprise_onboarding_workflow() {
    // Given: New enterprise customer
    let onboarding = CustomerOnboarding::new();
    let customer_id = CustomerId::new("enterprise_006");
    let customer = create_customer("enterprise_006", Tier::Enterprise, false);

    // Step 1: Register customer
    onboarding.register_customer(customer).unwrap();

    // Step 2: Verify no PHI access without BAA
    let customer_without_baa = onboarding.get_customer(&customer_id).unwrap();
    let result = onboarding.enable_phi_features(&customer_without_baa).await;
    assert!(matches!(result, Err(HipaaError::BaaRequired)));

    // Step 3: Sign BAA
    let signature = onboarding
        .sign_baa_for_customer(
            &customer_id,
            "Legal Representative".to_string(),
            "sha256:complete_hash".to_string(),
        )
        .unwrap();

    assert!(signature.signed_at <= chrono::Utc::now());

    // Step 4: Verify BAA is recorded
    assert!(onboarding.has_signed_baa(&customer_id).unwrap());

    // Step 5: Enable PHI features
    let customer_with_baa = onboarding.get_customer(&customer_id).unwrap();
    assert!(customer_with_baa.baa_signed);

    let result = onboarding.enable_phi_features(&customer_with_baa).await;
    assert!(result.is_ok(), "Complete workflow should enable PHI access");
}
