//! AES Coprocessor Test Suite
//!
//! Tests the AES-128 encryption coprocessor using NIST FIPS 197 test vectors
//! and validates session key management with 128 independent key slots.

use cognitum_coprocessor::{
    aes::{AesCoprocessor, SessionKeyManager},
    types::{CryptoError, Key128, Result},
};

/// NIST FIPS 197 Test Vector - AES-128 ECB Mode
#[tokio::test]
async fn test_aes128_ecb_nist_vector() {
    // Test vector from NIST FIPS 197, Appendix C.1
    let key = Key128::from_bytes([
        0x2b, 0x7e, 0x15, 0x16, 0x28, 0xae, 0xd2, 0xa6, 0xab, 0xf7, 0x15, 0x88, 0x09, 0xcf, 0x4f,
        0x3c,
    ]);

    let plaintext = [
        0x6b, 0xc1, 0xbe, 0xe2, 0x2e, 0x40, 0x9f, 0x96, 0xe9, 0x3d, 0x7e, 0x11, 0x73, 0x93, 0x17,
        0x2a,
    ];

    let expected_ciphertext = [
        0x3a, 0xd7, 0x7b, 0xb4, 0x0d, 0x7a, 0x36, 0x60, 0xa8, 0x9e, 0xca, 0xf3, 0x24, 0x66, 0xef,
        0x97,
    ];

    let mut aes = AesCoprocessor::new();

    // Encrypt single block
    let ciphertext = aes.encrypt_block(&key, &plaintext).await.unwrap();

    assert_eq!(
        ciphertext, expected_ciphertext,
        "AES-128 ECB encryption failed NIST test vector"
    );
}

/// Test multiple NIST test vectors
#[tokio::test]
async fn test_aes128_multiple_vectors() {
    let test_cases = vec![
        // Vector 1
        (
            [0x00; 16], // All-zero key
            [0x00; 16], // All-zero plaintext
            [
                0x66, 0xe9, 0x4b, 0xd4, 0xef, 0x8a, 0x2c, 0x3b, 0x88, 0x4c, 0xfa, 0x59, 0xca, 0x34,
                0x2b, 0x2e,
            ],
        ),
        // Vector 2
        (
            [0xff; 16], // All-ones key
            [0xff; 16], // All-ones plaintext
            [
                0x3f, 0x5b, 0x8c, 0xc9, 0xea, 0x85, 0x5a, 0x0a, 0xfa, 0x73, 0x47, 0xd2, 0x3e, 0x8d,
                0x66, 0x4e,
            ],
        ),
    ];

    let mut aes = AesCoprocessor::new();

    for (i, (key_bytes, plaintext, expected)) in test_cases.iter().enumerate() {
        let key = Key128::from_bytes(*key_bytes);
        let ciphertext = aes.encrypt_block(&key, plaintext).await.unwrap();

        assert_eq!(
            &ciphertext,
            expected,
            "AES-128 test vector {} failed",
            i + 1
        );
    }
}

/// Test session key management with 128 independent key slots
#[tokio::test]
async fn test_session_key_management() {
    let device_key = Key128::from_bytes([0x42; 16]);
    let mut key_mgr = SessionKeyManager::new(&device_key);

    // Derive keys for all 128 slots
    for slot in 0..128 {
        let session_id = [slot as u8; 16];
        let result = key_mgr.derive_session_key(slot, &session_id).await;

        assert!(
            result.is_ok(),
            "Failed to derive session key for slot {}",
            slot
        );
    }

    // Verify keys are unique
    let key1 = key_mgr.get_key(0).await.unwrap();
    let key2 = key_mgr.get_key(1).await.unwrap();

    // Keys must be different
    assert_ne!(
        unsafe { key1.expose_secret() },
        unsafe { key2.expose_secret() },
        "Session keys must be unique"
    );
}

/// Test key slot allocation and revocation
#[tokio::test]
async fn test_key_revocation() {
    let device_key = Key128::from_bytes([0x99; 16]);
    let mut key_mgr = SessionKeyManager::new(&device_key);

    // Allocate key in slot 5
    let session_id = [0x05; 16];
    key_mgr.derive_session_key(5, &session_id).await.unwrap();

    // Verify key exists
    assert!(key_mgr.get_key(5).await.is_ok());

    // Revoke key
    key_mgr.revoke_session(5).await;

    // Verify key is gone
    let result = key_mgr.get_key(5).await;
    assert!(result.is_err(), "Revoked key should not be accessible");
}

/// Test AES encryption latency simulation (~14 cycles)
#[tokio::test]
async fn test_aes_encryption_latency() {
    let key = Key128::from_bytes([0x11; 16]);
    let plaintext = [0x22; 16];

    let mut aes = AesCoprocessor::new();

    let start = std::time::Instant::now();
    let _ciphertext = aes.encrypt_block(&key, &plaintext).await.unwrap();
    let duration = start.elapsed();

    // In simulation, we add artificial delay to simulate ~14 cycle latency
    // At 1GHz, this is ~14ns, but we simulate with microseconds for testing
    assert!(
        duration.as_micros() >= 10,
        "Encryption should simulate hardware latency"
    );
}

/// Test pipelined 4-word burst mode
#[tokio::test]
async fn test_aes_burst_mode() {
    let key = Key128::from_bytes([0xaa; 16]);
    let blocks = vec![[0x01; 16], [0x02; 16], [0x03; 16], [0x04; 16]];

    let mut aes = AesCoprocessor::new();

    // Encrypt in burst mode (simulates pipeline)
    let results = aes.encrypt_burst(&key, &blocks).await.unwrap();

    assert_eq!(results.len(), 4, "Should encrypt all 4 blocks");

    // Verify each result is unique
    assert_ne!(results[0], results[1]);
    assert_ne!(results[1], results[2]);
    assert_ne!(results[2], results[3]);
}

/// Test ECC error handling (simulated)
#[tokio::test]
async fn test_ecc_error_handling() {
    let mut aes = AesCoprocessor::new();

    // Simulate single-bit error (should auto-correct)
    aes.simulate_single_bit_error(true);

    let key = Key128::from_bytes([0xcc; 16]);
    let plaintext = [0xdd; 16];

    // Should still work (auto-corrected)
    let result = aes.encrypt_block(&key, &plaintext).await;
    assert!(result.is_ok(), "Single-bit ECC error should auto-correct");

    // Simulate double-bit error (should fail)
    aes.simulate_double_bit_error(true);

    let result = aes.encrypt_block(&key, &plaintext).await;
    assert!(
        matches!(result, Err(CryptoError::EccError)),
        "Double-bit ECC error should fail"
    );
}

/// Test counter increment for GCM mode
#[tokio::test]
async fn test_ivc_counter_increment() {
    let mut aes = AesCoprocessor::new();

    let key = Key128::from_bytes([0xee; 16]);
    let mut iv = [0u8; 16];
    iv[15] = 0xff; // Set counter to overflow

    // Encrypt with counter increment enabled
    aes.enable_counter_increment(true);

    let _result = aes.encrypt_with_iv(&key, &[0x11; 16], &iv).await.unwrap();

    // Verify counter incremented
    let new_iv = aes.get_current_iv();
    assert_eq!(new_iv[15], 0x00, "Counter should have incremented");
    assert_eq!(new_iv[14], 0x01, "Counter overflow should propagate");
}

/// Property-based test: Encryption is deterministic
#[tokio::test]
async fn test_encryption_determinism() {
    let key = Key128::from_bytes([0x42; 16]);
    let plaintext = [0x99; 16];

    let mut aes = AesCoprocessor::new();

    // Encrypt same data twice
    let ct1 = aes.encrypt_block(&key, &plaintext).await.unwrap();
    let ct2 = aes.encrypt_block(&key, &plaintext).await.unwrap();

    assert_eq!(
        ct1, ct2,
        "Same key and plaintext must produce same ciphertext"
    );
}

/// Property-based test: Different keys produce different ciphertexts
#[tokio::test]
async fn test_key_uniqueness() {
    let key1 = Key128::from_bytes([0x01; 16]);
    let key2 = Key128::from_bytes([0x02; 16]);
    let plaintext = [0x00; 16];

    let mut aes = AesCoprocessor::new();

    let ct1 = aes.encrypt_block(&key1, &plaintext).await.unwrap();
    let ct2 = aes.encrypt_block(&key2, &plaintext).await.unwrap();

    assert_ne!(
        ct1, ct2,
        "Different keys must produce different ciphertexts"
    );
}

/// Test concurrent operations (async safety)
#[tokio::test]
async fn test_concurrent_encryption() {
    let key = Key128::from_bytes([0x55; 16]);
    let aes = std::sync::Arc::new(tokio::sync::Mutex::new(AesCoprocessor::new()));

    // Spawn 10 concurrent encryption tasks
    let mut handles = vec![];
    for i in 0..10 {
        let aes_clone = aes.clone();
        let key_clone = key.clone();
        let handle = tokio::spawn(async move {
            let mut aes_lock = aes_clone.lock().await;
            let plaintext = [i as u8; 16];
            aes_lock.encrypt_block(&key_clone, &plaintext).await
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent encryption should succeed");
    }
}
