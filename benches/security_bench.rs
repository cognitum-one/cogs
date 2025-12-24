//! Security Operation Benchmarks
//!
//! Target performance:
//! - API key validation: < 1ms
//! - JWT verification: < 500μs
//! - Argon2 hashing: consistent timing (security requirement)

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, Algorithm, Version, Params,
};
use sha2::{Sha256, Digest};
use subtle::ConstantTimeEq;

fn bench_argon2_hashing(c: &mut Criterion) {
    let mut group = c.benchmark_group("argon2_hashing");

    // Default Argon2id configuration
    group.bench_function("default_config", |b| {
        let argon2 = Argon2::default();
        let password = b"sk_live_test_key_1234567890abcdef";

        b.iter(|| {
            let salt = SaltString::generate(&mut OsRng);
            let hash = argon2
                .hash_password(black_box(password), &salt)
                .unwrap();
            black_box(hash);
        });
    });

    // Fast configuration (lower security, better performance)
    group.bench_function("fast_config", |b| {
        let params = Params::new(1024, 1, 1, None).unwrap();
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let password = b"sk_live_test_key_1234567890abcdef";

        b.iter(|| {
            let salt = SaltString::generate(&mut OsRng);
            let hash = argon2
                .hash_password(black_box(password), &salt)
                .unwrap();
            black_box(hash);
        });
    });

    // Production configuration (high security)
    group.bench_function("production_config", |b| {
        let params = Params::new(65536, 3, 4, None).unwrap();
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let password = b"sk_live_test_key_1234567890abcdef";

        b.iter(|| {
            let salt = SaltString::generate(&mut OsRng);
            let hash = argon2
                .hash_password(black_box(password), &salt)
                .unwrap();
            black_box(hash);
        });
    });

    group.finish();
}

fn bench_argon2_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("argon2_verification");

    // Pre-generate hash for verification benchmarks
    let argon2 = Argon2::default();
    let password = b"sk_live_test_key_1234567890abcdef";
    let salt = SaltString::generate(&mut OsRng);
    let hash = argon2.hash_password(password, &salt).unwrap();
    let hash_string = hash.to_string();

    // Verification (constant-time operation)
    group.bench_function("verify_correct_password", |b| {
        let parsed_hash = PasswordHash::new(&hash_string).unwrap();

        b.iter(|| {
            let result = argon2.verify_password(black_box(password), &parsed_hash);
            black_box(result);
        });
    });

    // Verification with wrong password (should take same time)
    group.bench_function("verify_wrong_password", |b| {
        let parsed_hash = PasswordHash::new(&hash_string).unwrap();
        let wrong_password = b"sk_live_wrong_key_0000000000000000";

        b.iter(|| {
            let result = argon2.verify_password(black_box(wrong_password), &parsed_hash);
            black_box(result);
        });
    });

    group.finish();
}

fn bench_api_key_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("api_key_validation");

    // Format validation (fast path)
    group.bench_function("format_validation", |b| {
        let key = "sk_live_1234567890abcdef1234567890abcdef";

        b.iter(|| {
            let valid = black_box(key).starts_with("sk_live_") && key.len() >= 40;
            black_box(valid);
        });
    });

    // Key derivation (SHA256)
    group.bench_function("key_derivation", |b| {
        let key = "sk_live_1234567890abcdef1234567890abcdef";

        b.iter(|| {
            let hash = Sha256::digest(black_box(key).as_bytes());
            let key_id = format!("key_{}", hex::encode(&hash[..16]));
            black_box(key_id);
        });
    });

    // Full validation pipeline (format + derivation + hash verification)
    group.bench_function("full_validation", |b| {
        let argon2 = Argon2::default();
        let key = "sk_live_1234567890abcdef1234567890abcdef";
        let salt = SaltString::generate(&mut OsRng);
        let stored_hash = argon2.hash_password(key.as_bytes(), &salt).unwrap().to_string();
        let parsed_hash = PasswordHash::new(&stored_hash).unwrap();

        b.iter(|| {
            // 1. Format validation
            if !key.starts_with("sk_live_") {
                return false;
            }

            // 2. Derive key ID
            let hash = Sha256::digest(key.as_bytes());
            let _key_id = format!("key_{}", hex::encode(&hash[..16]));

            // 3. Verify hash
            let valid = argon2.verify_password(black_box(key.as_bytes()), &parsed_hash).is_ok();
            black_box(valid)
        });
    });

    group.finish();
}

fn bench_constant_time_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("constant_time_comparison");

    let data1 = [0u8; 32];
    let data2_same = [0u8; 32];
    let data2_diff = [1u8; 32];

    // Constant-time equal comparison (same data)
    group.bench_function("ct_eq_same", |b| {
        b.iter(|| {
            let equal = black_box(&data1).ct_eq(black_box(&data2_same));
            black_box(equal);
        });
    });

    // Constant-time equal comparison (different data)
    group.bench_function("ct_eq_different", |b| {
        b.iter(|| {
            let equal = black_box(&data1).ct_eq(black_box(&data2_diff));
            black_box(equal);
        });
    });

    // Non-constant-time comparison (for reference - DO NOT USE in production)
    group.bench_function("regular_eq_same", |b| {
        b.iter(|| {
            let equal = black_box(&data1) == black_box(&data2_same);
            black_box(equal);
        });
    });

    group.bench_function("regular_eq_different", |b| {
        b.iter(|| {
            let equal = black_box(&data1) == black_box(&data2_diff);
            black_box(equal);
        });
    });

    group.finish();
}

fn bench_cryptographic_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("crypto_operations");

    // SHA256 hashing
    group.bench_function("sha256_hash", |b| {
        let data = b"sk_live_1234567890abcdef1234567890abcdef";

        b.iter(|| {
            let hash = Sha256::digest(black_box(data));
            black_box(hash);
        });
    });

    // Random key generation
    group.bench_function("random_key_gen", |b| {
        b.iter(|| {
            let mut key_bytes = [0u8; 32];
            getrandom::getrandom(&mut key_bytes).unwrap();
            black_box(key_bytes);
        });
    });

    // Hex encoding
    group.bench_function("hex_encode", |b| {
        let data = [0xABu8; 32];

        b.iter(|| {
            let encoded = hex::encode(black_box(&data));
            black_box(encoded);
        });
    });

    // Hex decoding
    group.bench_function("hex_decode", |b| {
        let encoded = "abababababababababababababababababababababababababababababababab";

        b.iter(|| {
            let decoded = hex::decode(black_box(encoded)).unwrap();
            black_box(decoded);
        });
    });

    group.finish();
}

fn bench_jwt_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("jwt_simulation");

    // Simulate JWT header.payload parsing
    group.bench_function("parse_jwt_components", |b| {
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1c2VyXzEyMyIsImV4cCI6MTYxNjE2MTYxNn0.signature";

        b.iter(|| {
            let parts: Vec<&str> = black_box(jwt).split('.').collect();
            black_box(parts);
        });
    });

    // Simulate JWT signature verification (SHA256-based)
    group.bench_function("verify_jwt_signature", |b| {
        let header_payload = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1c2VyXzEyMyIsImV4cCI6MTYxNjE2MTYxNn0";
        let secret = b"secret_key_1234567890";

        b.iter(|| {
            use sha2::Digest;
            let mut hasher = Sha256::new();
            hasher.update(black_box(header_payload).as_bytes());
            hasher.update(black_box(secret));
            let signature = hasher.finalize();
            black_box(signature);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_argon2_hashing,
    bench_argon2_verification,
    bench_api_key_validation,
    bench_constant_time_comparison,
    bench_cryptographic_operations,
    bench_jwt_simulation,
);
criterion_main!(benches);
