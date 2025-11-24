//! Comprehensive Cryptographic Coprocessor Benchmarks
//!
//! Benchmarks all crypto operations and compares against software implementations

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use cognitum_coprocessor::{
    aes::AesCoprocessor, puf::PhysicalUF, sha256::Sha256Coprocessor, trng::TrngCoprocessor,
    types::Key128,
};
use tokio::runtime::Runtime;

/// Software AES implementation for comparison
fn software_aes_encrypt(key: &[u8; 16], plaintext: &[u8; 16]) -> [u8; 16] {
    use aes::cipher::{BlockEncrypt, KeyInit};
    use aes::Aes128;

    let cipher = Aes128::new_from_slice(key).unwrap();
    let mut block = aes::Block::clone_from_slice(plaintext);
    cipher.encrypt_block(&mut block);
    block.into()
}

/// Software SHA-256 implementation for comparison
fn software_sha256(data: &[u8]) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

/// Benchmark AES-128 encryption (single block)
fn bench_aes_single_block(c: &mut Criterion) {
    let mut group = c.benchmark_group("aes_single_block");
    let rt = Runtime::new().unwrap();

    let key = Key128::from_bytes([0u8; 16]);
    let plaintext = [0u8; 16];

    // Hardware AES
    group.bench_function("hardware", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut aes = AesCoprocessor::new();
                black_box(aes.encrypt_block(&key, &plaintext).await.unwrap())
            })
        })
    });

    // Software AES (for comparison)
    group.bench_function("software", |b| {
        b.iter(|| {
            black_box(software_aes_encrypt(
                unsafe { key.expose_secret() },
                &plaintext,
            ))
        })
    });

    group.finish();
}

/// Benchmark AES-128 burst mode (4 blocks)
fn bench_aes_burst(c: &mut Criterion) {
    let mut group = c.benchmark_group("aes_burst");
    let rt = Runtime::new().unwrap();

    let key = Key128::from_bytes([0u8; 16]);
    let blocks = [[0u8; 16]; 4];

    group.throughput(Throughput::Bytes(64)); // 4 blocks * 16 bytes

    // Hardware AES burst
    group.bench_function("hardware_burst", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut aes = AesCoprocessor::new();
                black_box(aes.encrypt_burst(&key, &blocks).await.unwrap())
            })
        })
    });

    // Software AES sequential
    group.bench_function("software_sequential", |b| {
        b.iter(|| {
            let mut results = Vec::with_capacity(4);
            for block in &blocks {
                results.push(black_box(software_aes_encrypt(
                    unsafe { key.expose_secret() },
                    block,
                )));
            }
            black_box(results)
        })
    });

    group.finish();
}

/// Benchmark SHA-256 with various data sizes
fn bench_sha256(c: &mut Criterion) {
    let mut group = c.benchmark_group("sha256");
    let rt = Runtime::new().unwrap();

    // Test various sizes: 64B, 512B, 4KB, 64KB, 1MB
    for size in [64, 512, 4096, 65536, 1048576].iter() {
        let data = vec![0u8; *size];

        group.throughput(Throughput::Bytes(*size as u64));

        // Hardware SHA-256
        group.bench_with_input(BenchmarkId::new("hardware", size), &data, |b, data| {
            b.iter(|| {
                rt.block_on(async {
                    let mut sha = Sha256Coprocessor::new();
                    black_box(sha.hash(data).await.unwrap())
                })
            })
        });

        // Software SHA-256
        group.bench_with_input(BenchmarkId::new("software", size), &data, |b, data| {
            b.iter(|| black_box(software_sha256(data)))
        });
    }

    group.finish();
}

/// Benchmark SHA-256 streaming mode
fn bench_sha256_streaming(c: &mut Criterion) {
    let mut group = c.benchmark_group("sha256_streaming");
    let rt = Runtime::new().unwrap();

    let chunk1 = vec![0u8; 512];
    let chunk2 = vec![1u8; 512];
    let chunk3 = vec![2u8; 512];

    group.throughput(Throughput::Bytes(1536)); // 3 chunks * 512 bytes

    group.bench_function("hardware_streaming", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut sha = Sha256Coprocessor::new();
                sha.update(&chunk1).await.unwrap();
                sha.update(&chunk2).await.unwrap();
                sha.update(&chunk3).await.unwrap();
                black_box(sha.finalize().await.unwrap())
            })
        })
    });

    group.bench_function("software_streaming", |b| {
        b.iter(|| {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(&chunk1);
            hasher.update(&chunk2);
            hasher.update(&chunk3);
            black_box(hasher.finalize())
        })
    });

    group.finish();
}

/// Benchmark HMAC-SHA256
fn bench_hmac(c: &mut Criterion) {
    let mut group = c.benchmark_group("hmac_sha256");
    let rt = Runtime::new().unwrap();

    let key = b"secret_key_for_hmac_testing_here";
    let message = b"The quick brown fox jumps over the lazy dog";

    group.throughput(Throughput::Bytes(message.len() as u64));

    group.bench_function("hardware", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut sha = Sha256Coprocessor::new();
                black_box(sha.hmac(key, message).await.unwrap())
            })
        })
    });

    group.bench_function("software", |b| {
        b.iter(|| {
            use sha2::{Digest, Sha256};

            const BLOCK_SIZE: usize = 64;
            const OPAD: u8 = 0x5C;
            const IPAD: u8 = 0x36;

            let mut key_block = [0u8; BLOCK_SIZE];
            key_block[..key.len()].copy_from_slice(key);

            // Inner hash
            let mut hasher = Sha256::new();
            for &byte in key_block.iter() {
                hasher.update(&[byte ^ IPAD]);
            }
            hasher.update(message);
            let inner_hash = hasher.finalize();

            // Outer hash
            let mut hasher = Sha256::new();
            for &byte in key_block.iter() {
                hasher.update(&[byte ^ OPAD]);
            }
            hasher.update(&inner_hash);
            black_box(hasher.finalize())
        })
    });

    group.finish();
}

/// Benchmark TRNG throughput
fn bench_trng(c: &mut Criterion) {
    let mut group = c.benchmark_group("trng");
    let rt = Runtime::new().unwrap();

    // Single u32 generation
    group.bench_function("generate_u32", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut trng = TrngCoprocessor::new();
                black_box(trng.generate_u32().await.unwrap())
            })
        })
    });

    // Fill buffer (1KB)
    group.throughput(Throughput::Bytes(1024));
    group.bench_function("fill_1kb", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut trng = TrngCoprocessor::new();
                let mut buffer = [0u8; 1024];
                trng.fill_bytes(&mut buffer).await.unwrap();
                black_box(buffer)
            })
        })
    });

    // Compare with software RNG
    group.bench_function("software_rng_1kb", |b| {
        b.iter(|| {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let mut buffer = [0u8; 1024];
            rng.fill(&mut buffer[..]);
            black_box(buffer)
        })
    });

    group.finish();
}

/// Benchmark TRNG startup test
fn bench_trng_startup(c: &mut Criterion) {
    let mut group = c.benchmark_group("trng_startup");
    let rt = Runtime::new().unwrap();

    group.bench_function("startup_test", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut trng = TrngCoprocessor::new();
                black_box(trng.run_startup_test().await.unwrap())
            })
        })
    });

    group.finish();
}

/// Benchmark PUF challenge-response
fn bench_puf_challenge(c: &mut Criterion) {
    let mut group = c.benchmark_group("puf_challenge_response");
    let rt = Runtime::new().unwrap();

    let chip_seed = 0x123456789ABCDEF0;

    // Single challenge-response
    group.bench_function("single_crp", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut puf = PhysicalUF::new(chip_seed);
                black_box(puf.challenge_response(0x42).await.unwrap())
            })
        })
    });

    // Challenge-response with noise
    group.bench_function("crp_with_noise", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut puf = PhysicalUF::new(chip_seed);
                puf.enable_noise(true, 0.1); // 10% noise
                black_box(puf.challenge_response(0x42).await.unwrap())
            })
        })
    });

    group.finish();
}

/// Benchmark PUF device key derivation
fn bench_puf_key_derivation(c: &mut Criterion) {
    let mut group = c.benchmark_group("puf_key_derivation");
    let rt = Runtime::new().unwrap();

    let chip_seed = 0x123456789ABCDEF0;

    group.bench_function("derive_device_key", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut puf = PhysicalUF::new(chip_seed);
                black_box(puf.derive_device_key().await.unwrap())
            })
        })
    });

    // Helper data generation
    group.bench_function("generate_helper_data", |b| {
        b.iter(|| {
            rt.block_on(async {
                let puf = PhysicalUF::new(chip_seed);
                let response = 0xDEADBEEFCAFEBABE;
                black_box(puf.generate_helper_data(response).await.unwrap())
            })
        })
    });

    // Key reconstruction
    group.bench_function("reconstruct_key", |b| {
        b.iter(|| {
            rt.block_on(async {
                let puf = PhysicalUF::new(chip_seed);
                let noisy = 0xDEADBEEFCAFEBABE;
                let helper = vec![0u8; 32];
                black_box(puf.reconstruct_key(noisy, &helper).await.unwrap())
            })
        })
    });

    group.finish();
}

/// Benchmark AES session key operations
fn bench_session_keys(c: &mut Criterion) {
    let mut group = c.benchmark_group("session_keys");
    let rt = Runtime::new().unwrap();

    let device_key = Key128::from_bytes([1u8; 16]);
    let session_id = [0u8; 16];

    group.bench_function("derive_session_key", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut mgr = cognitum_coprocessor::aes::SessionKeyManager::new(&device_key);
                black_box(mgr.derive_session_key(0, &session_id).await.unwrap())
            })
        })
    });

    group.bench_function("get_session_key", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut mgr = cognitum_coprocessor::aes::SessionKeyManager::new(&device_key);
                mgr.derive_session_key(0, &session_id).await.unwrap();
                black_box(mgr.get_key(0).await.unwrap())
            })
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_aes_single_block,
    bench_aes_burst,
    bench_sha256,
    bench_sha256_streaming,
    bench_hmac,
    bench_trng,
    bench_trng_startup,
    bench_puf_challenge,
    bench_puf_key_derivation,
    bench_session_keys,
);

criterion_main!(benches);
