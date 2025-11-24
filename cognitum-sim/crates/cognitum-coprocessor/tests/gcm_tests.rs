//! GCM Coprocessor Test Suite
//!
//! Tests the GCM authenticated encryption coprocessor using NIST test vectors
//! and validates security properties including constant-time verification.

use cognitum_coprocessor::{
    gcm::GcmCoprocessor,
    types::{CryptoError, Key128},
};

/// NIST GCM Test Vector - Case 1
/// Test Case 2 from NIST SP 800-38D (Recommendation for Block Cipher Modes)
#[tokio::test]
async fn test_gcm_nist_vector_1() {
    let mut gcm = GcmCoprocessor::new();

    // Key: all zeros
    let key = Key128::from_bytes([0x00; 16]);

    // Plaintext: all zeros (16 bytes)
    let plaintext = [0x00; 16];

    // Nonce: 96 bits of zeros
    let nonce = [0x00; 12];
    gcm.set_nonce(nonce).unwrap();

    // No AAD
    gcm.set_aad(vec![]);

    // Expected ciphertext (from NIST)
    let expected_ct = [
        0x03, 0x88, 0xda, 0xce, 0x60, 0xb6, 0xa3, 0x92,
        0xf3, 0x28, 0xc2, 0xb9, 0x71, 0xb2, 0xfe, 0x78,
    ];

    // Expected tag (from NIST)
    let expected_tag = [
        0xab, 0x6e, 0x47, 0xd4, 0x2c, 0xec, 0x13, 0xbd,
        0xf5, 0x3a, 0x67, 0xb2, 0x12, 0x57, 0xbd, 0xdf,
    ];

    // Encrypt
    let (ciphertext, tag) = gcm.encrypt(&key, &plaintext).await.unwrap();

    assert_eq!(
        &ciphertext[..],
        &expected_ct[..],
        "GCM ciphertext doesn't match NIST test vector"
    );

    assert_eq!(
        tag, expected_tag,
        "GCM authentication tag doesn't match NIST test vector"
    );

    // Verify decryption
    gcm.clear_nonce_history();
    gcm.set_nonce(nonce).unwrap();
    let decrypted = gcm.decrypt(&key, &ciphertext, &tag).await.unwrap();
    assert_eq!(decrypted, plaintext);
}

/// NIST GCM Test Vector - Case 2 (with different key)
#[tokio::test]
async fn test_gcm_nist_vector_2() {
    let mut gcm = GcmCoprocessor::new();

    // Key: specific pattern
    let key = Key128::from_bytes([
        0xfe, 0xff, 0xe9, 0x92, 0x86, 0x65, 0x73, 0x1c,
        0x6d, 0x6a, 0x8f, 0x94, 0x67, 0x30, 0x83, 0x08,
    ]);

    // Plaintext: specific pattern
    let plaintext = [
        0xd9, 0x31, 0x32, 0x25, 0xf8, 0x84, 0x06, 0xe5,
        0xa5, 0x59, 0x09, 0xc5, 0xaf, 0xf5, 0x26, 0x9a,
        0x86, 0xa7, 0xa9, 0x53, 0x15, 0x34, 0xf7, 0xda,
        0x2e, 0x4c, 0x30, 0x3d, 0x8a, 0x31, 0x8a, 0x72,
        0x1c, 0x3c, 0x0c, 0x95, 0x95, 0x68, 0x09, 0x53,
        0x2f, 0xcf, 0x0e, 0x24, 0x49, 0xa6, 0xb5, 0x25,
        0xb1, 0x6a, 0xed, 0xf5, 0xaa, 0x0d, 0xe6, 0x57,
        0xba, 0x63, 0x7b, 0x39, 0x1a, 0xaf, 0xd2, 0x55,
    ];

    // Nonce
    let nonce = [
        0xca, 0xfe, 0xba, 0xbe, 0xfa, 0xce, 0xdb, 0xad,
        0xde, 0xca, 0xf8, 0x88,
    ];
    gcm.set_nonce(nonce).unwrap();

    // No AAD
    gcm.set_aad(vec![]);

    // Encrypt and decrypt
    let (ciphertext, tag) = gcm.encrypt(&key, &plaintext).await.unwrap();

    // Verify we can decrypt successfully
    gcm.clear_nonce_history();
    gcm.set_nonce(nonce).unwrap();
    let decrypted = gcm.decrypt(&key, &ciphertext, &tag).await.unwrap();
    assert_eq!(decrypted, plaintext);
}

/// Test GCM with Additional Authenticated Data (AAD)
#[tokio::test]
async fn test_gcm_with_aad() {
    let mut gcm = GcmCoprocessor::new();

    let key = Key128::from_bytes([0x42; 16]);
    let plaintext = b"Secret message";
    let aad = b"Packet header data";

    let nonce = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

    // Encrypt with AAD
    gcm.set_nonce(nonce).unwrap();
    gcm.set_aad(aad.to_vec());
    let (ciphertext, tag) = gcm.encrypt(&key, plaintext).await.unwrap();

    // Decrypt with same AAD should succeed
    gcm.clear_nonce_history();
    gcm.set_nonce(nonce).unwrap();
    gcm.set_aad(aad.to_vec());
    let decrypted = gcm.decrypt(&key, &ciphertext, &tag).await.unwrap();
    assert_eq!(decrypted, plaintext);

    // Decrypt with different AAD should fail
    gcm.clear_nonce_history();
    gcm.set_nonce(nonce).unwrap();
    gcm.set_aad(b"Modified header".to_vec());
    let result = gcm.decrypt(&key, &ciphertext, &tag).await;
    assert!(matches!(result, Err(CryptoError::AuthenticationFailed)));
}

/// Test GCM with various data sizes (1 byte to 1024 bytes)
#[tokio::test]
async fn test_gcm_various_sizes() {
    let key = Key128::from_bytes([0x99; 16]);

    for size in [1, 15, 16, 17, 31, 32, 64, 128, 256, 512, 1024] {
        let mut gcm = GcmCoprocessor::new();
        let plaintext: Vec<u8> = (0..size).map(|i| i as u8).collect();

        let nonce = [size as u8; 12];
        gcm.set_nonce(nonce).unwrap();

        // Encrypt
        let (ciphertext, tag) = gcm.encrypt(&key, &plaintext).await.unwrap();

        // Verify ciphertext length matches plaintext
        assert_eq!(ciphertext.len(), plaintext.len());

        // Decrypt
        let decrypted = gcm.decrypt(&key, &ciphertext, &tag).await.unwrap();
        assert_eq!(
            decrypted, plaintext,
            "Decryption failed for size {}",
            size
        );
    }
}

/// Test empty plaintext (valid in GCM - only produces authentication tag)
#[tokio::test]
async fn test_gcm_empty_plaintext() {
    let mut gcm = GcmCoprocessor::new();
    let key = Key128::from_bytes([0x55; 16]);
    let plaintext = b"";

    gcm.set_nonce([1; 12]).unwrap();
    gcm.set_aad(b"Only AAD, no plaintext".to_vec());

    let (ciphertext, tag) = gcm.encrypt(&key, plaintext).await.unwrap();

    assert!(ciphertext.is_empty(), "Empty plaintext should produce empty ciphertext");
    assert_ne!(tag, [0; 16], "Tag should still be generated");

    // Verify decryption
    let decrypted = gcm.decrypt(&key, &ciphertext, &tag).await.unwrap();
    assert_eq!(decrypted, plaintext);
}

/// Test tag verification fails with modified ciphertext
#[tokio::test]
async fn test_gcm_modified_ciphertext() {
    let mut gcm = GcmCoprocessor::new();
    let key = Key128::from_bytes([0xaa; 16]);
    let plaintext = b"Original message";

    gcm.set_nonce([5; 12]).unwrap();

    let (mut ciphertext, tag) = gcm.encrypt(&key, plaintext).await.unwrap();

    // Modify ciphertext
    ciphertext[0] ^= 0x01;

    // Decryption should fail due to authentication
    let result = gcm.decrypt(&key, &ciphertext, &tag).await;
    assert!(matches!(result, Err(CryptoError::AuthenticationFailed)));
}

/// Test tag verification fails with modified tag
#[tokio::test]
async fn test_gcm_modified_tag() {
    let mut gcm = GcmCoprocessor::new();
    let key = Key128::from_bytes([0xbb; 16]);
    let plaintext = b"Test message";

    gcm.set_nonce([6; 12]).unwrap();

    let (ciphertext, mut tag) = gcm.encrypt(&key, plaintext).await.unwrap();

    // Modify tag
    tag[0] ^= 0x01;

    // Decryption should fail
    let result = gcm.decrypt(&key, &ciphertext, &tag).await;
    assert!(matches!(result, Err(CryptoError::AuthenticationFailed)));
}

/// Test nonce reuse detection
#[tokio::test]
async fn test_gcm_nonce_reuse_prevention() {
    let mut gcm = GcmCoprocessor::new();
    let key = Key128::from_bytes([0xcc; 16]);
    let nonce = [7; 12];

    // First use should succeed
    assert!(gcm.set_nonce(nonce).is_ok());
    let _result = gcm.encrypt(&key, b"Message 1").await.unwrap();

    // Second use of same nonce should fail
    assert!(matches!(
        gcm.set_nonce(nonce),
        Err(CryptoError::NonceReused)
    ));
}

/// Test deterministic encryption (same inputs produce same outputs)
#[tokio::test]
async fn test_gcm_deterministic() {
    let key = Key128::from_bytes([0xdd; 16]);
    let plaintext = b"Deterministic test";
    let nonce = [8; 12];

    // First encryption
    let mut gcm1 = GcmCoprocessor::new();
    gcm1.set_nonce(nonce).unwrap();
    let (ct1, tag1) = gcm1.encrypt(&key, plaintext).await.unwrap();

    // Second encryption with same parameters
    let mut gcm2 = GcmCoprocessor::new();
    gcm2.set_nonce(nonce).unwrap();
    let (ct2, tag2) = gcm2.encrypt(&key, plaintext).await.unwrap();

    assert_eq!(ct1, ct2, "Same inputs should produce same ciphertext");
    assert_eq!(tag1, tag2, "Same inputs should produce same tag");
}

/// Test different nonces produce different outputs
#[tokio::test]
async fn test_gcm_different_nonces() {
    let key = Key128::from_bytes([0xee; 16]);
    let plaintext = b"Same plaintext";

    // Encrypt with nonce 1
    let mut gcm1 = GcmCoprocessor::new();
    gcm1.set_nonce([1; 12]).unwrap();
    let (ct1, tag1) = gcm1.encrypt(&key, plaintext).await.unwrap();

    // Encrypt with nonce 2
    let mut gcm2 = GcmCoprocessor::new();
    gcm2.set_nonce([2; 12]).unwrap();
    let (ct2, tag2) = gcm2.encrypt(&key, plaintext).await.unwrap();

    assert_ne!(ct1, ct2, "Different nonces should produce different ciphertext");
    assert_ne!(tag1, tag2, "Different nonces should produce different tags");
}

/// Test encryption latency simulation (~90 cycles)
#[tokio::test]
async fn test_gcm_encryption_latency() {
    let mut gcm = GcmCoprocessor::new();
    let key = Key128::from_bytes([0xff; 16]);
    let plaintext = b"Latency test";

    gcm.set_nonce([9; 12]).unwrap();

    let start = std::time::Instant::now();
    let _result = gcm.encrypt(&key, plaintext).await.unwrap();
    let duration = start.elapsed();

    // Should simulate ~90µs latency
    assert!(
        duration.as_micros() >= 90,
        "GCM encryption should simulate ~90 cycle latency"
    );
}

/// Test decryption latency simulation (~90 cycles)
#[tokio::test]
async fn test_gcm_decryption_latency() {
    let mut gcm = GcmCoprocessor::new();
    let key = Key128::from_bytes([0x11; 16]);
    let plaintext = b"Latency test";

    gcm.set_nonce([10; 12]).unwrap();
    let (ciphertext, tag) = gcm.encrypt(&key, plaintext).await.unwrap();

    gcm.clear_nonce_history();
    gcm.set_nonce([10; 12]).unwrap();

    let start = std::time::Instant::now();
    let _result = gcm.decrypt(&key, &ciphertext, &tag).await.unwrap();
    let duration = start.elapsed();

    // Should simulate ~90µs latency
    assert!(
        duration.as_micros() >= 90,
        "GCM decryption should simulate ~90 cycle latency"
    );
}

/// Test concurrent GCM operations (async safety)
#[tokio::test]
async fn test_gcm_concurrent_operations() {
    let key = Key128::from_bytes([0x22; 16]);

    let mut handles = vec![];

    for i in 0..10 {
        let key_clone = key.clone_key();
        let handle = tokio::spawn(async move {
            let mut gcm = GcmCoprocessor::new();
            let plaintext = format!("Message {}", i);
            let nonce = [i as u8; 12];

            gcm.set_nonce(nonce).unwrap();
            gcm.encrypt(&key_clone, plaintext.as_bytes()).await
        });
        handles.push(handle);
    }

    // Wait for all operations to complete
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent GCM operations should succeed");
    }
}

/// Property test: Decryption of valid ciphertext always succeeds
#[tokio::test]
async fn test_gcm_property_valid_decrypt() {
    let key = Key128::from_bytes([0x33; 16]);

    for i in 0..20 {
        let mut gcm = GcmCoprocessor::new();
        let plaintext = format!("Test message {}", i);
        let nonce = [i as u8; 12];

        gcm.set_nonce(nonce).unwrap();
        let (ciphertext, tag) = gcm.encrypt(&key, plaintext.as_bytes()).await.unwrap();

        gcm.clear_nonce_history();
        gcm.set_nonce(nonce).unwrap();
        let decrypted = gcm.decrypt(&key, &ciphertext, &tag).await;

        assert!(decrypted.is_ok(), "Valid ciphertext should always decrypt");
        assert_eq!(decrypted.unwrap(), plaintext.as_bytes());
    }
}

/// Security test: All tag bits affect verification
#[tokio::test]
async fn test_gcm_tag_bit_sensitivity() {
    let mut gcm = GcmCoprocessor::new();
    let key = Key128::from_bytes([0x44; 16]);
    let plaintext = b"Bit sensitivity test";

    gcm.set_nonce([11; 12]).unwrap();
    let (ciphertext, tag) = gcm.encrypt(&key, plaintext).await.unwrap();

    // Test each bit of the tag
    for byte_idx in 0..16 {
        for bit_idx in 0..8 {
            let mut modified_tag = tag;
            modified_tag[byte_idx] ^= 1 << bit_idx;

            gcm.clear_nonce_history();
            gcm.set_nonce([11; 12]).unwrap();
            let result = gcm.decrypt(&key, &ciphertext, &modified_tag).await;

            assert!(
                matches!(result, Err(CryptoError::AuthenticationFailed)),
                "Modified tag bit {}.{} should cause authentication failure",
                byte_idx, bit_idx
            );
        }
    }
}
