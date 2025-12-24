//! Unit tests for cryptographic primitives
//!
//! Tests cover:
//! - AES-GCM unique nonce generation
//! - AES-GCM tamper detection
//! - Ed25519 deterministic signatures
//! - Argon2 timing consistency
//! - SecureRandom entropy validation

use cognitum_security::crypto::{AesGcmCipher, Argon2Config, Argon2Hasher, Ed25519Signer};
use cognitum_security::error::CryptoError;
use cognitum_security::random::SecureRandom;
use std::collections::HashSet;
use std::time::{Duration, Instant};

#[test]
fn aes_gcm_uses_unique_nonce_per_encryption() {
    let cipher = AesGcmCipher::new(&[0u8; 32]).unwrap();
    let plaintext = b"sensitive_data";

    // Encrypt same data twice
    let (ciphertext1, nonce1) = cipher.encrypt(plaintext).unwrap();
    let (ciphertext2, nonce2) = cipher.encrypt(plaintext).unwrap();

    // Nonces must be unique
    assert_ne!(nonce1, nonce2, "Nonces must be unique for each encryption");

    // Ciphertexts differ due to unique nonces
    assert_ne!(
        ciphertext1, ciphertext2,
        "Different nonces should produce different ciphertexts"
    );

    // Both should decrypt correctly
    let decrypted1 = cipher.decrypt(&ciphertext1, &nonce1).unwrap();
    let decrypted2 = cipher.decrypt(&ciphertext2, &nonce2).unwrap();
    assert_eq!(decrypted1, plaintext);
    assert_eq!(decrypted2, plaintext);
}

#[test]
fn aes_gcm_detects_tampering() {
    let cipher = AesGcmCipher::new(&[0u8; 32]).unwrap();
    let plaintext = b"critical_data";

    let (mut ciphertext, nonce) = cipher.encrypt(plaintext).unwrap();

    // Tamper with ciphertext
    ciphertext[0] ^= 0xFF;

    // Decryption must fail with authentication error
    let result = cipher.decrypt(&ciphertext, &nonce);
    assert!(
        matches!(result, Err(CryptoError::AuthenticationFailed)),
        "Tampered ciphertext should fail authentication"
    );
}

#[test]
fn aes_gcm_wrong_nonce_fails() {
    let cipher = AesGcmCipher::new(&[0u8; 32]).unwrap();
    let plaintext = b"test_data";

    let (ciphertext, _correct_nonce) = cipher.encrypt(plaintext).unwrap();
    let wrong_nonce = [0u8; 12]; // All zeros

    let result = cipher.decrypt(&ciphertext, &wrong_nonce);
    assert!(
        matches!(result, Err(CryptoError::AuthenticationFailed)),
        "Wrong nonce should fail decryption"
    );
}

#[test]
fn aes_gcm_encrypts_with_aad() {
    let cipher = AesGcmCipher::new(&[0u8; 32]).unwrap();
    let plaintext = b"secret message";
    let aad = b"user_id=12345";

    let (ciphertext, nonce) = cipher.encrypt_with_aad(plaintext, aad).unwrap();

    // Decrypt with correct AAD
    let decrypted = cipher.decrypt_with_aad(&ciphertext, &nonce, aad).unwrap();
    assert_eq!(decrypted, plaintext);

    // Decrypt with wrong AAD should fail
    let wrong_aad = b"user_id=99999";
    let result = cipher.decrypt_with_aad(&ciphertext, &nonce, wrong_aad);
    assert!(matches!(result, Err(CryptoError::AuthenticationFailed)));
}

#[test]
fn ed25519_signatures_are_deterministic() {
    let signer = Ed25519Signer::generate().unwrap();
    let message = b"license_token_data";

    let sig1 = signer.sign(message).unwrap();
    let sig2 = signer.sign(message).unwrap();

    // Ed25519 signatures are deterministic
    assert_eq!(sig1, sig2, "Ed25519 signatures must be deterministic");
}

#[test]
fn ed25519_verifies_correct_signatures() {
    let signer = Ed25519Signer::generate().unwrap();
    let message = b"important document";

    let signature = signer.sign(message).unwrap();

    // Correct signature should verify
    assert!(
        signer.verify(message, &signature).unwrap(),
        "Valid signature should verify"
    );

    // Wrong message should fail
    let wrong_message = b"tampered document";
    assert!(
        !signer.verify(wrong_message, &signature).unwrap(),
        "Signature should not verify with different message"
    );
}

#[test]
fn ed25519_invalid_signature_length() {
    let signer = Ed25519Signer::generate().unwrap();
    let message = b"test";

    // Signature with wrong length
    let bad_signature = vec![0u8; 32]; // Should be 64 bytes

    let result = signer.verify(message, &bad_signature).unwrap();
    assert!(!result, "Invalid signature length should fail verification");
}

#[test]
fn ed25519_from_bytes() {
    let private_key = [42u8; 32];
    let signer = Ed25519Signer::from_bytes(&private_key).unwrap();

    let message = b"test message";
    let signature = signer.sign(message).unwrap();

    assert!(signer.verify(message, &signature).unwrap());
}

#[test]
fn argon2_timing_is_consistent() {
    let hasher = Argon2Hasher::new(Argon2Config {
        memory_cost: 19456, // Lower for faster tests (19 MiB)
        time_cost: 2,
        parallelism: 1,
    });

    let password = "user_password_123";

    // Hash multiple times and measure
    let mut durations = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        let _ = hasher.hash(password).unwrap();
        durations.push(start.elapsed());
    }

    // Calculate average
    let total: Duration = durations.iter().sum();
    let avg = total / durations.len() as u32;

    // Timing should be consistent (within 20% variance for test environment)
    for d in &durations {
        let variance =
            (d.as_millis() as f64 - avg.as_millis() as f64).abs() / avg.as_millis() as f64;
        assert!(
            variance < 0.20,
            "Timing variance too high: {:.2}% (duration: {:?}, avg: {:?})",
            variance * 100.0,
            d,
            avg
        );
    }
}

#[test]
fn argon2_verifies_correct_password() {
    let hasher = Argon2Hasher::default();
    let password = "secure_password_456";

    let hash = hasher.hash(password).unwrap();

    // Correct password should verify
    assert!(
        hasher.verify(password, &hash).unwrap(),
        "Correct password should verify"
    );

    // Wrong password should fail
    assert!(
        !hasher.verify("wrong_password", &hash).unwrap(),
        "Wrong password should not verify"
    );
}

#[test]
fn argon2_different_salts_produce_different_hashes() {
    let hasher = Argon2Hasher::default();
    let password = "same_password";

    let hash1 = hasher.hash(password).unwrap();
    let hash2 = hasher.hash(password).unwrap();

    // Different salts should produce different hashes
    assert_ne!(hash1, hash2, "Different salts should produce different hashes");

    // Both should verify
    assert!(hasher.verify(password, &hash1).unwrap());
    assert!(hasher.verify(password, &hash2).unwrap());
}

#[test]
fn argon2_constant_time_comparison() {
    let correct = b"api_key_12345678901234567890";
    let wrong = b"api_key_99999999999999999999";

    // Same values should match
    assert!(Argon2Hasher::verify_constant_time(correct, correct));

    // Different values should not match
    assert!(!Argon2Hasher::verify_constant_time(correct, wrong));

    // Different lengths
    let short = b"short";
    assert!(!Argon2Hasher::verify_constant_time(correct, short));
}

#[test]
fn secure_random_passes_entropy_check() {
    let rng = SecureRandom::new();
    let mut bytes = [0u8; 256];
    rng.fill(&mut bytes).unwrap();

    // Basic entropy check: all bytes should not be same
    let unique_bytes: HashSet<_> = bytes.iter().collect();
    assert!(
        unique_bytes.len() > 200,
        "Insufficient entropy: only {} unique bytes",
        unique_bytes.len()
    );

    // Should not be all zeros or all ones
    assert_ne!(bytes, [0u8; 256]);
    assert_ne!(bytes, [0xFFu8; 256]);

    // Chi-square test for randomness
    let chi_square = calculate_chi_square(&bytes);
    assert!(
        chi_square > 200.0 && chi_square < 330.0,
        "Chi-square {} outside expected range [200, 330]",
        chi_square
    );
}

#[test]
fn secure_random_generates_unique_nonces() {
    let rng = SecureRandom::new();
    let mut nonces = HashSet::new();

    for _ in 0..100 {
        let nonce = rng.generate_nonce().unwrap();
        assert!(
            nonces.insert(nonce),
            "Generated duplicate nonce (collision)"
        );
    }

    assert_eq!(nonces.len(), 100);
}

#[test]
fn secure_random_generates_unique_salts() {
    let rng = SecureRandom::new();
    let mut salts = HashSet::new();

    for _ in 0..100 {
        let salt = rng.generate_salt().unwrap();
        assert!(salts.insert(salt), "Generated duplicate salt (collision)");
    }

    assert_eq!(salts.len(), 100);
}

#[test]
fn secure_random_api_keys_are_unique() {
    let rng = SecureRandom::new();
    let mut keys = HashSet::new();

    for _ in 0..100 {
        let key = rng.generate_api_key().unwrap();
        assert!(!key.is_empty());
        // Should be URL-safe base64
        assert!(key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
        assert!(keys.insert(key), "Generated duplicate API key");
    }

    assert_eq!(keys.len(), 100);
}

// Helper function: Chi-square test for randomness
fn calculate_chi_square(data: &[u8]) -> f64 {
    let mut counts = [0u32; 256];
    for &byte in data {
        counts[byte as usize] += 1;
    }

    let expected = data.len() as f64 / 256.0;
    let mut chi_square = 0.0;

    for &count in &counts {
        let diff = count as f64 - expected;
        chi_square += (diff * diff) / expected;
    }

    chi_square
}

#[test]
fn test_chi_square_calculation() {
    // Perfectly uniform distribution
    let uniform = vec![1u8; 256];
    let chi = calculate_chi_square(&uniform);
    assert!(chi < 10.0, "Uniform distribution should have low chi-square");

    // All same value (very non-random)
    let non_random = vec![42u8; 256];
    let chi = calculate_chi_square(&non_random);
    assert!(
        chi > 1000.0,
        "Non-random distribution should have high chi-square"
    );
}
