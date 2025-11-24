//! SHA-256 Coprocessor Test Suite
//!
//! Tests SHA-256 hashing coprocessor using NIST FIPS 180-4 test vectors
//! and validates 3-stage pipeline execution.

use cognitum_coprocessor::{
    sha256::Sha256Coprocessor,
    types::{Hash256, Result},
};

/// NIST FIPS 180-4 Test Vector - "abc"
#[tokio::test]
async fn test_sha256_abc() {
    let input = b"abc";
    let expected = [
        0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae, 0x22,
        0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61, 0xf2, 0x00,
        0x15, 0xad,
    ];

    let mut sha256 = Sha256Coprocessor::new();
    let hash = sha256.hash(input).await.unwrap();

    assert_eq!(
        hash.as_bytes(),
        &expected,
        "SHA-256('abc') failed NIST test vector"
    );
}

/// NIST FIPS 180-4 Test Vector - Empty string
#[tokio::test]
async fn test_sha256_empty() {
    let input = b"";
    let expected = [
        0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f, 0xb9,
        0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b, 0x78, 0x52,
        0xb8, 0x55,
    ];

    let mut sha256 = Sha256Coprocessor::new();
    let hash = sha256.hash(input).await.unwrap();

    assert_eq!(
        hash.as_bytes(),
        &expected,
        "SHA-256('') failed NIST test vector"
    );
}

/// NIST FIPS 180-4 Test Vector - Long message
#[tokio::test]
async fn test_sha256_long_message() {
    let input = b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq";
    let expected = [
        0x24, 0x8d, 0x6a, 0x61, 0xd2, 0x06, 0x38, 0xb8, 0xe5, 0xc0, 0x26, 0x93, 0x0c, 0x3e, 0x60,
        0x39, 0xa3, 0x3c, 0xe4, 0x59, 0x64, 0xff, 0x21, 0x67, 0xf6, 0xec, 0xed, 0xd4, 0x19, 0xdb,
        0x06, 0xc1,
    ];

    let mut sha256 = Sha256Coprocessor::new();
    let hash = sha256.hash(input).await.unwrap();

    assert_eq!(
        hash.as_bytes(),
        &expected,
        "SHA-256 long message failed NIST test vector"
    );
}

/// Test one million 'a' characters (NIST test)
#[tokio::test]
async fn test_sha256_million_a() {
    let input = vec![b'a'; 1_000_000];
    let expected = [
        0xcd, 0xc7, 0x6e, 0x5c, 0x99, 0x14, 0xfb, 0x92, 0x81, 0xa1, 0xc7, 0xe2, 0x84, 0xd7, 0x3e,
        0x67, 0xf1, 0x80, 0x9a, 0x48, 0xa4, 0x97, 0x20, 0x0e, 0x04, 0x6d, 0x39, 0xcc, 0xc7, 0x11,
        0x2c, 0xd0,
    ];

    let mut sha256 = Sha256Coprocessor::new();
    let hash = sha256.hash(&input).await.unwrap();

    assert_eq!(
        hash.as_bytes(),
        &expected,
        "SHA-256 million 'a' test failed"
    );
}

/// Test 512-bit block processing
#[tokio::test]
async fn test_sha256_block_processing() {
    let mut sha256 = Sha256Coprocessor::new();

    // Exactly one 512-bit block (64 bytes)
    let input = [0x42u8; 64];
    let hash1 = sha256.hash(&input).await.unwrap();

    // Two blocks (128 bytes)
    let input2 = [0x42u8; 128];
    let hash2 = sha256.hash(&input2).await.unwrap();

    // Hashes should be different
    assert_ne!(
        hash1.as_bytes(),
        hash2.as_bytes(),
        "Different inputs must produce different hashes"
    );
}

/// Test 3-stage pipeline simulation (~70 cycles per block)
#[tokio::test]
async fn test_sha256_pipeline_latency() {
    let mut sha256 = Sha256Coprocessor::new();
    let input = b"test data";

    let start = std::time::Instant::now();
    let _hash = sha256.hash(input).await.unwrap();
    let duration = start.elapsed();

    // Should simulate ~70 cycle latency (in microseconds for testing)
    assert!(
        duration.as_micros() >= 50,
        "SHA-256 should simulate 3-stage pipeline latency"
    );
}

/// Test streaming hash (multiple updates)
#[tokio::test]
async fn test_sha256_streaming() {
    let mut sha256 = Sha256Coprocessor::new();

    // Hash in three parts
    sha256.update(b"Hello, ").await.unwrap();
    sha256.update(b"World").await.unwrap();
    sha256.update(b"!").await.unwrap();

    let hash = sha256.finalize().await.unwrap();

    // Compare with single-shot hash
    let mut sha256_single = Sha256Coprocessor::new();
    let expected = sha256_single.hash(b"Hello, World!").await.unwrap();

    assert_eq!(
        hash.as_bytes(),
        expected.as_bytes(),
        "Streaming hash should match single-shot hash"
    );
}

/// Test hash determinism
#[tokio::test]
async fn test_sha256_determinism() {
    let input = b"deterministic test";

    let mut sha256_1 = Sha256Coprocessor::new();
    let hash1 = sha256_1.hash(input).await.unwrap();

    let mut sha256_2 = Sha256Coprocessor::new();
    let hash2 = sha256_2.hash(input).await.unwrap();

    assert_eq!(
        hash1.as_bytes(),
        hash2.as_bytes(),
        "Same input must produce same hash"
    );
}

/// Test avalanche effect (single bit change)
#[tokio::test]
async fn test_sha256_avalanche() {
    let mut sha256 = Sha256Coprocessor::new();

    let input1 = b"test";
    let input2 = b"Test"; // Only first bit different

    let hash1 = sha256.hash(input1).await.unwrap();
    let hash2 = sha256.hash(input2).await.unwrap();

    // Count differing bits
    let mut diff_bits = 0;
    for i in 0..32 {
        diff_bits += (hash1.as_bytes()[i] ^ hash2.as_bytes()[i]).count_ones();
    }

    // SHA-256 avalanche: ~50% of bits should differ
    assert!(
        diff_bits > 100 && diff_bits < 156,
        "Avalanche effect: expected ~128 bit differences, got {}",
        diff_bits
    );
}

/// Test HMAC-SHA256 (for key derivation)
#[tokio::test]
async fn test_hmac_sha256() {
    let mut sha256 = Sha256Coprocessor::new();

    let key = b"secret_key";
    let message = b"message to authenticate";

    let mac = sha256.hmac(key, message).await.unwrap();

    // HMAC should be 256 bits
    assert_eq!(mac.as_bytes().len(), 32, "HMAC-SHA256 should be 32 bytes");

    // Verify determinism
    let mac2 = sha256.hmac(key, message).await.unwrap();
    assert_eq!(
        mac.as_bytes(),
        mac2.as_bytes(),
        "HMAC should be deterministic"
    );
}

/// Test HMAC with different keys produces different MACs
#[tokio::test]
async fn test_hmac_key_sensitivity() {
    let mut sha256 = Sha256Coprocessor::new();

    let key1 = b"key1";
    let key2 = b"key2";
    let message = b"same message";

    let mac1 = sha256.hmac(key1, message).await.unwrap();
    let mac2 = sha256.hmac(key2, message).await.unwrap();

    assert_ne!(
        mac1.as_bytes(),
        mac2.as_bytes(),
        "Different keys must produce different HMACs"
    );
}

/// Test concurrent hashing operations
#[tokio::test]
async fn test_concurrent_hashing() {
    let sha256 = std::sync::Arc::new(tokio::sync::Mutex::new(Sha256Coprocessor::new()));

    // Spawn 20 concurrent hash operations
    let mut handles = vec![];
    for i in 0..20 {
        let sha256_clone = sha256.clone();
        let handle = tokio::spawn(async move {
            let mut sha_lock = sha256_clone.lock().await;
            let data = format!("data_{}", i);
            sha_lock.hash(data.as_bytes()).await
        });
        handles.push(handle);
    }

    // All should complete successfully
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "Concurrent hashing should succeed");
    }
}

/// Test performance: throughput measurement
#[tokio::test]
async fn test_sha256_throughput() {
    let mut sha256 = Sha256Coprocessor::new();
    let data = vec![0x42u8; 1024 * 1024]; // 1 MB

    let start = std::time::Instant::now();
    let _hash = sha256.hash(&data).await.unwrap();
    let duration = start.elapsed();

    let throughput_mbps = (data.len() as f64) / duration.as_secs_f64() / 1_000_000.0;

    println!("SHA-256 throughput: {:.2} MB/s", throughput_mbps);

    // Should achieve reasonable throughput (simulated)
    assert!(
        throughput_mbps > 10.0,
        "SHA-256 throughput too low: {:.2} MB/s",
        throughput_mbps
    );
}
