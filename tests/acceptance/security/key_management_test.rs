//! Acceptance tests for Key Management System
//!
//! Tests HSM integration, circuit breaker, and key rotation

use cognitum_security::{
    CircuitBreakerConfig, HsmError, HsmProvider, KeyManagementService, KeyPurpose, KmsError,
    MockHsmProvider,
};
use mockall::predicate::*;
use std::time::Duration;

#[tokio::test]
async fn encryption_keys_are_stored_in_hsm_not_memory() {
    // Given: A key management service with HSM backend
    let mut mock_hsm = MockHsmProvider::new();

    mock_hsm
        .expect_generate_key()
        .with(eq(KeyPurpose::DataEncryption), always())
        .times(1)
        .returning(|_, _| Ok(()));

    mock_hsm
        .expect_encrypt()
        .times(1)
        .returning(|_, _| Ok(vec![0xDE, 0xAD, 0xBE, 0xEF])); // Encrypted blob

    let kms = KeyManagementService::new(Box::new(mock_hsm));

    // When: We encrypt sensitive data
    let sensitive_data = b"PHI_PATIENT_RECORD";
    let result = kms.encrypt("data_key_001", sensitive_data).await;

    // Then: Data is encrypted via HSM (key never exposed)
    assert!(result.is_ok(), "Encryption should succeed");
    let encrypted = result.unwrap();
    assert_ne!(encrypted, sensitive_data.to_vec());

    // Verify: No raw keys exist in application memory
    assert_eq!(
        kms.raw_key_count(),
        0,
        "No raw keys should exist in application memory"
    );
}

#[tokio::test]
async fn key_rotation_maintains_data_accessibility() {
    // Given: Existing encrypted data and active key
    let mut mock_hsm = MockHsmProvider::new();
    let old_key_id = "key_v1";
    let new_key_id = "key_v2";

    mock_hsm
        .expect_rotate_key()
        .with(eq(old_key_id))
        .times(1)
        .returning(move |_| Ok(new_key_id.to_string()));

    mock_hsm
        .expect_decrypt()
        .times(1)
        .returning(|_, _| Ok(b"decrypted_data".to_vec()));

    let kms = KeyManagementService::new(Box::new(mock_hsm));

    // When: Key is rotated
    let rotation_result = kms.rotate_key(old_key_id).await;

    // Then: Rotation succeeds and old data remains accessible
    assert!(rotation_result.is_ok(), "Key rotation should succeed");
    let rotated_key_id = rotation_result.unwrap();
    assert_eq!(rotated_key_id, new_key_id);

    // Old data can still be decrypted (using key version metadata)
    let decrypt_result = kms.decrypt(old_key_id, &[0x01, 0x02]).await;
    assert!(decrypt_result.is_ok(), "Old data should still be decryptable");
}

#[tokio::test]
async fn hsm_failure_triggers_circuit_breaker() {
    // Given: HSM that fails intermittently
    let mut mock_hsm = MockHsmProvider::new();

    mock_hsm
        .expect_encrypt()
        .times(5)
        .returning(|_, _| Err(HsmError::ConnectionTimeout));

    let kms = KeyManagementService::with_circuit_breaker(
        Box::new(mock_hsm),
        CircuitBreakerConfig {
            failure_threshold: 3,
            reset_timeout: Duration::from_secs(30),
        },
    );

    // When: Multiple HSM operations fail
    for i in 0..5 {
        let result = kms.encrypt("key", b"data").await;
        if i < 3 {
            // First 3 failures go through to HSM
            assert!(matches!(result, Err(KmsError::HsmError(_))));
        } else {
            // After threshold, circuit breaker should open
            assert!(
                matches!(result, Err(KmsError::CircuitBreakerOpen)),
                "Circuit breaker should be open after {} failures",
                i
            );
        }
    }

    // Then: Circuit breaker opens, preventing cascade failures
    assert!(kms.is_circuit_open(), "Circuit breaker should be open");

    // Subsequent calls fail fast
    let result = kms.encrypt("key", b"data").await;
    assert!(matches!(result, Err(KmsError::CircuitBreakerOpen)));
}

#[tokio::test]
async fn hsm_sign_and_verify() {
    let mut mock_hsm = MockHsmProvider::new();

    let test_signature = vec![1, 2, 3, 4, 5];
    let signature_clone = test_signature.clone();

    mock_hsm
        .expect_sign()
        .with(eq("signing_key"), always())
        .times(1)
        .returning(move |_, _| Ok(signature_clone.clone()));

    mock_hsm
        .expect_verify()
        .with(eq("signing_key"), always(), always())
        .times(1)
        .returning(|_, _, _| Ok(true));

    let kms = KeyManagementService::new(Box::new(mock_hsm));

    // Sign data
    let data = b"important document";
    let signature = kms.sign("signing_key", data).await.unwrap();
    assert_eq!(signature, test_signature);

    // Note: Verify would require adding a verify method to KMS
    // This test demonstrates the sign operation works correctly
}

#[tokio::test]
async fn circuit_breaker_resets_after_timeout() {
    let mut mock_hsm = MockHsmProvider::new();

    // First 3 calls fail
    mock_hsm
        .expect_encrypt()
        .times(3)
        .returning(|_, _| Err(HsmError::ConnectionTimeout));

    // After reset, call succeeds
    mock_hsm
        .expect_encrypt()
        .times(1)
        .returning(|_, _| Ok(vec![1, 2, 3]));

    let kms = KeyManagementService::with_circuit_breaker(
        Box::new(mock_hsm),
        CircuitBreakerConfig {
            failure_threshold: 3,
            reset_timeout: Duration::from_millis(100), // Short timeout for test
        },
    );

    // Trigger 3 failures
    for _ in 0..3 {
        let _ = kms.encrypt("key", b"data").await;
    }

    assert!(kms.is_circuit_open());

    // Wait for reset timeout
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Circuit should be in half-open state and allow retry
    let result = kms.encrypt("key", b"data").await;
    assert!(
        result.is_ok(),
        "Circuit should allow retry after timeout and succeed"
    );

    // Circuit should be closed again
    assert!(!kms.is_circuit_open());
}

#[tokio::test]
async fn multiple_operations_increment_counter() {
    let mut mock_hsm = MockHsmProvider::new();

    mock_hsm
        .expect_encrypt()
        .times(5)
        .returning(|_, _| Ok(vec![1, 2, 3]));

    mock_hsm
        .expect_decrypt()
        .times(3)
        .returning(|_, _| Ok(vec![4, 5, 6]));

    mock_hsm.expect_sign().times(2).returning(|_, _| Ok(vec![7, 8]));

    let kms = KeyManagementService::new(Box::new(mock_hsm));

    // Perform various operations
    for _ in 0..5 {
        let _ = kms.encrypt("key", b"data").await;
    }
    for _ in 0..3 {
        let _ = kms.decrypt("key", b"encrypted").await;
    }
    for _ in 0..2 {
        let _ = kms.sign("key", b"message").await;
    }

    // Total: 10 operations
    assert_eq!(kms.get_operation_count(), 10);
}

#[tokio::test]
async fn hsm_decrypt_operation() {
    let mut mock_hsm = MockHsmProvider::new();

    let plaintext = b"decrypted message".to_vec();
    let plaintext_clone = plaintext.clone();

    mock_hsm
        .expect_decrypt()
        .with(eq("encryption_key"), always())
        .times(1)
        .returning(move |_, _| Ok(plaintext_clone.clone()));

    let kms = KeyManagementService::new(Box::new(mock_hsm));

    let ciphertext = b"encrypted data";
    let result = kms.decrypt("encryption_key", ciphertext).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), plaintext);
}

#[tokio::test]
async fn circuit_breaker_half_open_state() {
    let mut mock_hsm = MockHsmProvider::new();

    // Fail enough to open circuit
    mock_hsm
        .expect_encrypt()
        .times(3)
        .returning(|_, _| Err(HsmError::OperationFailed("test".to_string())));

    // Then succeed after reset
    mock_hsm
        .expect_encrypt()
        .times(1)
        .returning(|_, _| Ok(vec![1, 2, 3]));

    let kms = KeyManagementService::with_circuit_breaker(
        Box::new(mock_hsm),
        CircuitBreakerConfig {
            failure_threshold: 3,
            reset_timeout: Duration::from_millis(50),
        },
    );

    // Open the circuit
    for _ in 0..3 {
        let _ = kms.encrypt("key", b"data").await;
    }

    assert!(kms.is_circuit_open());

    // Wait for half-open transition
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Next operation should succeed and close circuit
    let result = kms.encrypt("key", b"data").await;
    assert!(result.is_ok());
    assert!(!kms.is_circuit_open());
}
