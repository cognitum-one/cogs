//! TRNG Coprocessor Test Suite
//!
//! Tests True Random Number Generator with NIST SP 800-90B compliance
//! and health monitoring validation.

use cognitum_coprocessor::{trng::TrngCoprocessor, types::Result};

/// Test basic random number generation
#[tokio::test]
async fn test_trng_generate_basic() {
    let mut trng = TrngCoprocessor::new();

    let random1 = trng.generate_u32().await.unwrap();
    let random2 = trng.generate_u32().await.unwrap();

    // Different calls should produce different values (extremely high probability)
    assert_ne!(
        random1, random2,
        "TRNG should produce different random values"
    );
}

/// Test random byte array generation
#[tokio::test]
async fn test_trng_fill_bytes() {
    let mut trng = TrngCoprocessor::new();
    let mut buffer = [0u8; 32];

    trng.fill_bytes(&mut buffer).await.unwrap();

    // Should not be all zeros
    assert_ne!(
        buffer, [0u8; 32],
        "TRNG should produce non-zero random bytes"
    );

    // Generate again and compare
    let mut buffer2 = [0u8; 32];
    trng.fill_bytes(&mut buffer2).await.unwrap();

    assert_ne!(
        buffer, buffer2,
        "TRNG should produce different random sequences"
    );
}

/// Test NIST health test monitoring
#[tokio::test]
async fn test_trng_health_monitoring() {
    let mut trng = TrngCoprocessor::new();

    // Enable health tests
    trng.enable_health_tests(true).await;

    // Generate random data
    for _ in 0..100 {
        let _random = trng.generate_u32().await.unwrap();
    }

    // Check health status
    let health_status = trng.get_health_status().await;
    assert!(health_status.is_healthy, "TRNG health tests should pass");
    assert_eq!(
        health_status.failures, 0,
        "No health test failures expected"
    );
}

/// Test Adaptive Proportion Test (APT)
#[tokio::test]
async fn test_trng_adaptive_proportion_test() {
    let mut trng = TrngCoprocessor::new();

    // Configure APT parameters
    trng.configure_apt(
        1024, // window_size
        512,  // cutoff
    )
    .await
    .unwrap();

    // Generate samples
    let mut samples = vec![];
    for _ in 0..2048 {
        samples.push(trng.generate_u32().await.unwrap() & 1); // LSB only
    }

    // Count zeros and ones
    let zeros = samples.iter().filter(|&&x| x == 0).count();
    let ones = samples.len() - zeros;

    // Should be roughly balanced (statistical test)
    let ratio = zeros as f64 / ones as f64;
    assert!(
        ratio > 0.8 && ratio < 1.2,
        "TRNG should produce balanced random bits (ratio: {:.2})",
        ratio
    );
}

/// Test Repetition Count Test (RCT)
#[tokio::test]
async fn test_trng_repetition_count_test() {
    let mut trng = TrngCoprocessor::new();

    // Configure RCT
    trng.configure_rct(32).await.unwrap(); // Max 32 consecutive repeats

    // Generate sequence and check for excessive repetition
    let mut prev = trng.generate_u32().await.unwrap();
    let mut max_repeats = 0;
    let mut current_repeats = 1;

    for _ in 0..1000 {
        let current = trng.generate_u32().await.unwrap();
        if current == prev {
            current_repeats += 1;
        } else {
            max_repeats = max_repeats.max(current_repeats);
            current_repeats = 1;
        }
        prev = current;
    }

    // Should not have excessive repetition
    assert!(
        max_repeats < 32,
        "TRNG should not have excessive repetition (max: {})",
        max_repeats
    );
}

/// Test startup self-test
#[tokio::test]
async fn test_trng_startup_self_test() {
    let mut trng = TrngCoprocessor::new();

    // Run startup test
    let startup_result = trng.run_startup_test().await.unwrap();

    assert!(startup_result.passed, "TRNG startup self-test should pass");
    assert!(
        startup_result.entropy_estimate > 0.0,
        "Should have entropy estimate"
    );
}

/// Test zeroization (security feature)
#[tokio::test]
async fn test_trng_zeroization() {
    let mut trng = TrngCoprocessor::new();

    // Generate some random data
    let _before = trng.generate_u32().await.unwrap();

    // Zeroize
    trng.zeroize().await.unwrap();

    // After zeroization, should still work but with fresh state
    let after = trng.generate_u32().await.unwrap();
    assert_ne!(after, 0, "TRNG should work after zeroization");
}

/// Test entropy estimation
#[tokio::test]
async fn test_trng_entropy_estimation() {
    let mut trng = TrngCoprocessor::new();

    // Collect samples
    let mut samples = vec![];
    for _ in 0..10000 {
        samples.push((trng.generate_u32().await.unwrap() & 0xFF) as u8);
    }

    // Calculate Shannon entropy
    let mut counts = [0usize; 256];
    for &byte in &samples {
        counts[byte as usize] += 1;
    }

    let mut entropy = 0.0;
    let n = samples.len() as f64;
    for &count in &counts {
        if count > 0 {
            let p = count as f64 / n;
            entropy -= p * p.log2();
        }
    }

    // Should be close to 8 bits per byte for good RNG
    assert!(
        entropy > 7.9,
        "TRNG entropy too low: {:.3} bits/byte",
        entropy
    );
}

/// Test chi-square randomness test
#[tokio::test]
async fn test_trng_chi_square() {
    let mut trng = TrngCoprocessor::new();

    // Generate samples
    let mut samples = vec![];
    for _ in 0..10000 {
        samples.push((trng.generate_u32().await.unwrap() % 100) as usize);
    }

    // Chi-square test
    let mut buckets = [0usize; 100];
    for &sample in &samples {
        buckets[sample] += 1;
    }

    let expected = samples.len() as f64 / 100.0;
    let mut chi_square = 0.0;
    for &count in &buckets {
        let diff = count as f64 - expected;
        chi_square += (diff * diff) / expected;
    }

    // Chi-square critical value for 99 degrees of freedom at 95% confidence ≈ 123.2
    assert!(
        chi_square < 140.0,
        "Chi-square test failed: {:.2} (should be < 140)",
        chi_square
    );
}

/// Test FIFO buffering
#[tokio::test]
async fn test_trng_fifo_buffering() {
    let mut trng = TrngCoprocessor::new();

    // Check FIFO status
    let status = trng.get_fifo_status().await;
    assert!(!status.is_full, "FIFO should not be full initially");

    // Fill FIFO with random data
    for _ in 0..32 {
        let _random = trng.generate_u32().await.unwrap();
    }

    // FIFO should have data
    let status = trng.get_fifo_status().await;
    assert!(status.count > 0, "FIFO should contain random data");
}

/// Test bypass modes (for debugging)
#[tokio::test]
async fn test_trng_bypass_mode() {
    let mut trng = TrngCoprocessor::new();

    // Normal mode
    let normal_random = trng.generate_u32().await.unwrap();

    // Enable CBC-MAC bypass
    trng.set_bypass_cbc(true).await;
    let bypass_random = trng.generate_u32().await.unwrap();

    // Both should produce random values (but different processing)
    assert_ne!(
        normal_random, bypass_random,
        "Bypass mode should produce different values"
    );

    // Disable bypass
    trng.set_bypass_cbc(false).await;
}

/// Test sampling frequency configuration
#[tokio::test]
async fn test_trng_sampling_frequency() {
    let mut trng = TrngCoprocessor::new();

    // Configure sampling frequency
    trng.set_sample_frequency(1_000_000).await.unwrap(); // 1 MHz
    trng.set_sample_divider(8).await.unwrap(); // Divide by 8

    // Generate random data
    let random = trng.generate_u32().await.unwrap();
    assert_ne!(random, 0, "TRNG should work with custom frequency");
}

/// Test interrupt generation on data ready
#[tokio::test]
async fn test_trng_interrupt() {
    let mut trng = TrngCoprocessor::new();

    // Clear interrupt flag
    trng.clear_interrupt().await;

    // Generate random number (should set interrupt)
    let _random = trng.generate_u32().await.unwrap();

    // Check interrupt flag
    assert!(
        trng.is_interrupt_pending().await,
        "Interrupt should be set after random generation"
    );

    // Clear it
    trng.clear_interrupt().await;
    assert!(
        !trng.is_interrupt_pending().await,
        "Interrupt should be cleared"
    );
}

/// Test concurrent TRNG access
#[tokio::test]
async fn test_concurrent_trng_access() {
    let trng = std::sync::Arc::new(tokio::sync::Mutex::new(TrngCoprocessor::new()));

    // Spawn multiple tasks
    let mut handles = vec![];
    for _ in 0..50 {
        let trng_clone = trng.clone();
        let handle = tokio::spawn(async move {
            let mut trng_lock = trng_clone.lock().await;
            trng_lock.generate_u32().await
        });
        handles.push(handle);
    }

    // Collect all results
    let mut results = vec![];
    for handle in handles {
        let result = handle.await.unwrap().unwrap();
        results.push(result);
    }

    // Check uniqueness (should be very high)
    results.sort();
    results.dedup();
    assert!(
        results.len() > 45,
        "Most concurrent TRNG values should be unique"
    );
}

/// Test performance: random number generation rate
#[tokio::test]
async fn test_trng_generation_rate() {
    let mut trng = TrngCoprocessor::new();

    let count = 10000;
    let start = std::time::Instant::now();

    for _ in 0..count {
        let _ = trng.generate_u32().await.unwrap();
    }

    let duration = start.elapsed();
    let rate = count as f64 / duration.as_secs_f64();

    println!("TRNG generation rate: {:.0} numbers/sec", rate);

    // Should achieve reasonable rate (simulated)
    assert!(
        rate > 1000.0,
        "TRNG generation rate too low: {:.0}/sec",
        rate
    );
}
