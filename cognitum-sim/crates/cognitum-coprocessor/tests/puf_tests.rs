//! PUF (Physical Unclonable Function) Coprocessor Test Suite
//!
//! Tests PUF challenge-response authentication and chip-unique key derivation.

use cognitum_coprocessor::{puf::PhysicalUF, types::Result};

/// Test basic PUF challenge-response
#[tokio::test]
async fn test_puf_challenge_response() {
    let puf = PhysicalUF::new(42); // Seed for simulation

    let challenge = 0x123456789ABCDEF0;
    let response = puf.challenge_response(challenge).await.unwrap();

    // Response should be non-zero
    assert_ne!(response, 0, "PUF should produce non-zero response");
}

/// Test PUF response consistency (same challenge = same response)
#[tokio::test]
async fn test_puf_consistency() {
    let puf = PhysicalUF::new(42);

    let challenge = 0xDEADBEEFCAFEBABE;

    // Query same challenge multiple times
    let r1 = puf.challenge_response(challenge).await.unwrap();
    let r2 = puf.challenge_response(challenge).await.unwrap();
    let r3 = puf.challenge_response(challenge).await.unwrap();

    assert_eq!(r1, r2, "PUF responses should be consistent");
    assert_eq!(r2, r3, "PUF responses should be consistent");
}

/// Test PUF uniqueness (different challenges = different responses)
#[tokio::test]
async fn test_puf_uniqueness() {
    let puf = PhysicalUF::new(42);

    let c1 = 0x0000000000000001;
    let c2 = 0x0000000000000002;

    let r1 = puf.challenge_response(c1).await.unwrap();
    let r2 = puf.challenge_response(c2).await.unwrap();

    assert_ne!(
        r1, r2,
        "Different challenges should produce different responses"
    );
}

/// Test PUF chip uniqueness (different seeds = different responses)
#[tokio::test]
async fn test_puf_chip_uniqueness() {
    let puf1 = PhysicalUF::new(42); // Chip 1
    let puf2 = PhysicalUF::new(43); // Chip 2

    let challenge = 0x1234567890ABCDEF;

    let r1 = puf1.challenge_response(challenge).await.unwrap();
    let r2 = puf2.challenge_response(challenge).await.unwrap();

    assert_ne!(r1, r2, "Different chips should produce different responses");
}

/// Test PUF with noise simulation (realistic behavior)
#[tokio::test]
async fn test_puf_with_noise() {
    let mut puf = PhysicalUF::new(42);

    // Enable noise simulation (5-15% bit error rate)
    puf.enable_noise(true, 0.10); // 10% noise

    let challenge = 0xABCDEF0123456789;

    // Collect multiple responses
    let mut responses = vec![];
    for _ in 0..10 {
        responses.push(puf.challenge_response(challenge).await.unwrap());
    }

    // Calculate Hamming distance between responses
    let mut total_distance = 0;
    for i in 1..responses.len() {
        let distance = (responses[0] ^ responses[i]).count_ones();
        total_distance += distance;
    }
    let avg_distance = total_distance as f64 / (responses.len() - 1) as f64;

    // With 10% noise, expect ~6.4 bit differences (64 bits * 0.10)
    assert!(
        avg_distance >= 3.0 && avg_distance <= 10.0,
        "PUF noise should cause ~{} bit errors, got {:.1}",
        6.4,
        avg_distance
    );
}

/// Test PUF helper data generation (for error correction)
#[tokio::test]
async fn test_puf_helper_data() {
    let puf = PhysicalUF::new(42);

    let challenge = 0x1122334455667788;
    let response = puf.challenge_response(challenge).await.unwrap();

    // Generate helper data for error correction
    let helper_data = puf.generate_helper_data(response).await.unwrap();

    assert!(helper_data.len() > 0, "Helper data should be generated");
    assert!(
        helper_data.len() >= 16,
        "Helper data should be sufficient for ECC"
    );
}

/// Test PUF key reconstruction with helper data
#[tokio::test]
async fn test_puf_key_reconstruction() {
    let mut puf = PhysicalUF::new(42);

    let challenge = 0x9988776655443322;

    // Enrollment: get clean response and helper data
    let clean_response = puf.challenge_response(challenge).await.unwrap();
    let helper_data = puf.generate_helper_data(clean_response).await.unwrap();

    // Enable noise for reconstruction
    puf.enable_noise(true, 0.10);

    // Reconstruction: get noisy response
    let noisy_response = puf.challenge_response(challenge).await.unwrap();

    // Reconstruct key using helper data
    let reconstructed = puf
        .reconstruct_key(noisy_response, &helper_data)
        .await
        .unwrap();

    assert_eq!(
        reconstructed, clean_response,
        "PUF should reconstruct original key despite noise"
    );
}

/// Test PUF-based device key derivation
#[tokio::test]
async fn test_puf_device_key_derivation() {
    let puf = PhysicalUF::new(42);

    // Derive 256-bit device key from PUF
    let device_key = puf.derive_device_key().await.unwrap();

    assert_eq!(device_key.len(), 32, "Device key should be 256 bits");

    // Derive again - should be identical
    let device_key2 = puf.derive_device_key().await.unwrap();
    assert_eq!(
        device_key, device_key2,
        "Device key derivation should be deterministic"
    );
}

/// Test PUF chip ID generation
#[tokio::test]
async fn test_puf_chip_id() {
    let puf1 = PhysicalUF::new(42);
    let puf2 = PhysicalUF::new(43);

    let id1 = puf1.get_chip_id().await.unwrap();
    let id2 = puf2.get_chip_id().await.unwrap();

    assert_ne!(id1, id2, "Different chips should have unique IDs");
}

/// Test PUF entropy quality
#[tokio::test]
async fn test_puf_entropy() {
    let puf = PhysicalUF::new(42);

    // Collect responses from different challenges
    let mut responses = vec![];
    for i in 0..256 {
        let response = puf.challenge_response(i).await.unwrap();
        responses.push(response);
    }

    // Calculate bit balance (should be ~50% zeros, 50% ones)
    let mut total_ones = 0;
    for response in &responses {
        total_ones += response.count_ones();
    }

    let total_bits = responses.len() * 64;
    let ones_ratio = total_ones as f64 / total_bits as f64;

    assert!(
        ones_ratio > 0.45 && ones_ratio < 0.55,
        "PUF should have balanced entropy (ratio: {:.3})",
        ones_ratio
    );
}

/// Test PUF challenge avalanche effect
#[tokio::test]
async fn test_puf_avalanche() {
    let puf = PhysicalUF::new(42);

    let c1 = 0x0000000000000000;
    let c2 = 0x0000000000000001; // Single bit different

    let r1 = puf.challenge_response(c1).await.unwrap();
    let r2 = puf.challenge_response(c2).await.unwrap();

    // Count bit differences
    let diff_bits = (r1 ^ r2).count_ones();

    // Should have strong avalanche (many bits different)
    assert!(
        diff_bits > 20,
        "PUF should have avalanche effect (got {} bit differences)",
        diff_bits
    );
}

/// Test PUF tamper detection simulation
#[tokio::test]
async fn test_puf_tamper_detection() {
    let mut puf = PhysicalUF::new(42);

    let challenge = 0x1111222233334444;
    let original = puf.challenge_response(challenge).await.unwrap();

    // Simulate tampering (changes physical characteristics)
    puf.simulate_tamper();

    let after_tamper = puf.challenge_response(challenge).await.unwrap();

    // Response should be significantly different after tampering
    let diff_bits = (original ^ after_tamper).count_ones();
    assert!(
        diff_bits > 32,
        "Tampering should cause major PUF response change (got {} bits)",
        diff_bits
    );
}

/// Test PUF oscillator configuration
#[tokio::test]
async fn test_puf_oscillator_config() {
    let puf = PhysicalUF::new(42);

    // Configure oscillator parameters
    puf.configure_oscillators(
        4,  // channels
        36, // vector bits
    )
    .await
    .unwrap();

    let response = puf.challenge_response(0x123).await.unwrap();
    assert_ne!(response, 0, "PUF should work with configured oscillators");
}

/// Test PUF challenge-response pair (CRP) database
#[tokio::test]
async fn test_puf_crp_database() {
    let puf = PhysicalUF::new(42);

    // Generate CRP database
    let mut crp_db = vec![];
    for i in 0..128 {
        let challenge = i as u64;
        let response = puf.challenge_response(challenge).await.unwrap();
        crp_db.push((challenge, response));
    }

    // Verify all CRPs
    for (challenge, expected_response) in crp_db {
        let response = puf.challenge_response(challenge).await.unwrap();
        assert_eq!(
            response, expected_response,
            "CRP should be consistent for challenge 0x{:x}",
            challenge
        );
    }
}

/// Test PUF authentication protocol
#[tokio::test]
async fn test_puf_authentication() {
    let puf_device = PhysicalUF::new(42);
    let puf_verifier = PhysicalUF::new(42); // Same chip

    // Authentication challenge
    let challenge = 0xDEADBEEF;

    // Device responds
    let device_response = puf_device.challenge_response(challenge).await.unwrap();

    // Verifier checks
    let expected_response = puf_verifier.challenge_response(challenge).await.unwrap();

    assert_eq!(
        device_response, expected_response,
        "PUF authentication should succeed for same chip"
    );

    // Test with different chip (should fail)
    let puf_impostor = PhysicalUF::new(99);
    let impostor_response = puf_impostor.challenge_response(challenge).await.unwrap();

    assert_ne!(
        impostor_response, expected_response,
        "PUF authentication should fail for different chip"
    );
}

/// Test PUF key transfer to AES coprocessor
#[tokio::test]
async fn test_puf_key_transfer() {
    let puf = PhysicalUF::new(42);

    // Derive key from PUF
    let device_key = puf.derive_device_key().await.unwrap();

    // Transfer to AES (simulated via pmosi interface)
    let transfer_data = puf.prepare_key_transfer(&device_key).await.unwrap();

    assert_eq!(
        transfer_data.len(),
        48 / 8, // 48-bit pmosi interface
        "Key transfer data should match pmosi interface width"
    );
}

/// Test PUF performance: challenge-response latency
#[tokio::test]
async fn test_puf_latency() {
    let puf = PhysicalUF::new(42);

    let challenge = 0x1234567890ABCDEF;

    let start = std::time::Instant::now();
    let _response = puf.challenge_response(challenge).await.unwrap();
    let duration = start.elapsed();

    println!("PUF challenge-response latency: {:?}", duration);

    // Should be relatively fast (simulated)
    assert!(duration.as_micros() < 1000, "PUF latency should be < 1ms");
}
