//! Comprehensive XSalsa20 test suite with NaCl test vectors

use cognitum_coprocessor::xsalsa20::{XSalsa20, XSalsa20Key};

/// NaCl test vector 1
/// From: https://nacl.cr.yp.to/stream.html
#[tokio::test]
async fn test_nacl_vector_1() {
    let key = XSalsa20Key::from_bytes([
        0x1b, 0x27, 0x55, 0x64, 0x73, 0xe9, 0x85, 0xd4,
        0x62, 0xcd, 0x51, 0x19, 0x7a, 0x9a, 0x46, 0xc7,
        0x60, 0x09, 0x54, 0x9e, 0xac, 0x64, 0x74, 0xf2,
        0x06, 0xc4, 0xee, 0x08, 0x44, 0xf6, 0x83, 0x89,
    ]);

    let nonce = [
        0x69, 0x69, 0x6e, 0xe9, 0x55, 0xb6, 0x2b, 0x73,
        0xcd, 0x62, 0xbd, 0xa8, 0x75, 0xfc, 0x73, 0xd6,
        0x82, 0x19, 0xe0, 0x03, 0x6b, 0x7a, 0x0b, 0x37,
    ];

    let mut data = vec![0u8; 64];
    let mut cipher = XSalsa20::new(key, nonce);
    cipher.simulate_latency(false); // Disable for exact test
    cipher.encrypt(&mut data).await.unwrap();

    // Expected keystream (first 64 bytes)
    let expected = [
        0xee, 0xa6, 0xa7, 0x25, 0x1c, 0x1e, 0x72, 0x91,
        0x6d, 0x11, 0xc2, 0xcb, 0x21, 0x4d, 0x3c, 0x25,
        0x25, 0x39, 0x12, 0x1d, 0x8e, 0x23, 0x4e, 0x65,
        0x2d, 0x65, 0x1f, 0xa4, 0xc8, 0xcf, 0xf8, 0x80,
        0x30, 0x9e, 0x64, 0x5a, 0x74, 0xe9, 0xe0, 0xa6,
        0x0d, 0x82, 0x43, 0xac, 0xd9, 0x17, 0x7a, 0xb5,
        0x1a, 0x1b, 0xeb, 0x8d, 0x5a, 0x2f, 0x5d, 0x70,
        0x0c, 0x09, 0x3c, 0x5e, 0x55, 0x85, 0x57, 0x96,
    ];

    assert_eq!(data, expected);
}

/// NaCl test vector 2 - Different key/nonce
#[tokio::test]
async fn test_nacl_vector_2() {
    let key = XSalsa20Key::from_bytes([
        0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
        0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d, 0x8e, 0x8f,
        0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97,
        0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9d, 0x9e, 0x9f,
    ]);

    let nonce = [
        0x07, 0x00, 0x00, 0x00, 0x40, 0x41, 0x42, 0x43,
        0x44, 0x45, 0x46, 0x47, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    let mut data = vec![0u8; 64];
    let mut cipher = XSalsa20::new(key, nonce);
    cipher.simulate_latency(false);
    cipher.encrypt(&mut data).await.unwrap();

    // This is a partial keystream - verify non-zero
    assert_ne!(data, vec![0u8; 64]);
}

/// Test zero key and zero nonce
#[tokio::test]
async fn test_zero_key_nonce() {
    let key = XSalsa20Key::from_bytes([0; 32]);
    let nonce = [0; 24];

    let mut data = vec![0u8; 128];
    let mut cipher = XSalsa20::new(key, nonce);
    cipher.simulate_latency(false);
    cipher.encrypt(&mut data).await.unwrap();

    // Should produce non-zero keystream
    assert_ne!(data, vec![0u8; 128]);
}

/// Test encryption/decryption with real data
#[tokio::test]
async fn test_encrypt_decrypt_real_data() {
    let key = XSalsa20Key::from_bytes([
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
        0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
        0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20,
    ]);

    let nonce = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
        0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10,
        0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18,
    ];

    let plaintext = b"Lorem ipsum dolor sit amet, consectetur adipiscing elit. \
                      Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.";
    let mut data = plaintext.to_vec();
    let original = data.clone();

    // Encrypt
    let mut cipher = XSalsa20::new(XSalsa20Key::from_bytes(unsafe { *key.expose_secret() }), nonce);
    cipher.simulate_latency(false);
    cipher.encrypt(&mut data).await.unwrap();

    assert_ne!(data, original);

    // Decrypt
    let mut cipher2 = XSalsa20::new(XSalsa20Key::from_bytes(unsafe { *key.expose_secret() }), nonce);
    cipher2.simulate_latency(false);
    cipher2.decrypt(&mut data).await.unwrap();

    assert_eq!(data, original);
}

/// Test different nonces produce different output
#[tokio::test]
async fn test_nonce_variation() {
    let _key = XSalsa20Key::from_bytes([42; 32]);
    let mut data1 = vec![0u8; 64];
    let mut data2 = vec![0u8; 64];

    let nonce1 = [1; 24];
    let nonce2 = [2; 24];

    let mut cipher1 = XSalsa20::new(XSalsa20Key::from_bytes([42; 32]), nonce1);
    cipher1.simulate_latency(false);
    cipher1.encrypt(&mut data1).await.unwrap();

    let mut cipher2 = XSalsa20::new(XSalsa20Key::from_bytes([42; 32]), nonce2);
    cipher2.simulate_latency(false);
    cipher2.encrypt(&mut data2).await.unwrap();

    // Different nonces should produce different keystreams
    assert_ne!(data1, data2);
}

/// Test different keys produce different output
#[tokio::test]
async fn test_key_variation() {
    let nonce = [7; 24];
    let mut data1 = vec![0u8; 64];
    let mut data2 = vec![0u8; 64];

    let key1 = XSalsa20Key::from_bytes([1; 32]);
    let key2 = XSalsa20Key::from_bytes([2; 32]);

    let mut cipher1 = XSalsa20::new(key1, nonce);
    cipher1.simulate_latency(false);
    cipher1.encrypt(&mut data1).await.unwrap();

    let mut cipher2 = XSalsa20::new(key2, nonce);
    cipher2.simulate_latency(false);
    cipher2.encrypt(&mut data2).await.unwrap();

    // Different keys should produce different keystreams
    assert_ne!(data1, data2);
}

/// Test large data encryption (multiple blocks)
#[tokio::test]
async fn test_large_data() {
    let key = XSalsa20Key::from_bytes([0x55; 32]);
    let nonce = [0xaa; 24];

    let mut data = vec![0x42; 10000]; // ~156 blocks
    let original = data.clone();

    let mut cipher = XSalsa20::new(key, nonce);
    cipher.simulate_latency(false);
    cipher.encrypt(&mut data).await.unwrap();

    assert_ne!(data, original);

    let mut cipher2 = XSalsa20::new(XSalsa20Key::from_bytes([0x55; 32]), nonce);
    cipher2.simulate_latency(false);
    cipher2.decrypt(&mut data).await.unwrap();

    assert_eq!(data, original);
}

/// Test counter seek (random access)
#[tokio::test]
async fn test_counter_seek() {
    let _key = XSalsa20Key::from_bytes([0x33; 32]);
    let nonce = [0x66; 24];

    // Encrypt at counter 0
    let mut data1 = vec![0u8; 64];
    let mut cipher1 = XSalsa20::new(XSalsa20Key::from_bytes([0x33; 32]), nonce);
    cipher1.simulate_latency(false);
    cipher1.encrypt(&mut data1).await.unwrap();

    // Encrypt at counter 5
    let mut data2 = vec![0u8; 64];
    let mut cipher2 = XSalsa20::new(XSalsa20Key::from_bytes([0x33; 32]), nonce);
    cipher2.set_counter(5);
    cipher2.simulate_latency(false);
    cipher2.encrypt(&mut data2).await.unwrap();

    // Different counter positions should produce different output
    assert_ne!(data1, data2);
}

/// Test batch encryption consistency
#[tokio::test]
async fn test_batch_consistency() {
    let key = [0x77; 32];
    let nonce = [0x88; 24];

    // Sequential encryption
    let mut buf1 = b"First".to_vec();
    let mut buf2 = b"Second".to_vec();
    let mut buf3 = b"Third".to_vec();

    let mut cipher_seq = XSalsa20::new(XSalsa20Key::from_bytes(key), nonce);
    cipher_seq.simulate_latency(false);
    cipher_seq.encrypt(&mut buf1).await.unwrap();
    cipher_seq.encrypt(&mut buf2).await.unwrap();
    cipher_seq.encrypt(&mut buf3).await.unwrap();

    // Batch encryption
    let mut buf4 = b"First".to_vec();
    let mut buf5 = b"Second".to_vec();
    let mut buf6 = b"Third".to_vec();

    let mut cipher_batch = XSalsa20::new(XSalsa20Key::from_bytes(key), nonce);
    cipher_batch.simulate_latency(false);
    let mut buffers = [buf4.as_mut_slice(), buf5.as_mut_slice(), buf6.as_mut_slice()];
    cipher_batch.encrypt_batch(&mut buffers).await.unwrap();

    // Should produce identical results
    assert_eq!(buf1, buffers[0]);
    assert_eq!(buf2, buffers[1]);
    assert_eq!(buf3, buffers[2]);
}

/// Test edge case: empty buffer
#[tokio::test]
async fn test_empty_buffer() {
    let key = XSalsa20Key::from_bytes([0x99; 32]);
    let nonce = [0xaa; 24];

    let mut data = vec![];
    let mut cipher = XSalsa20::new(key, nonce);
    cipher.simulate_latency(false);

    // Should not panic
    cipher.encrypt(&mut data).await.unwrap();
    assert_eq!(data.len(), 0);
}

/// Test edge case: single byte
#[tokio::test]
async fn test_single_byte() {
    let key = XSalsa20Key::from_bytes([0xbb; 32]);
    let nonce = [0xcc; 24];

    let mut data = vec![0x42];
    let original = data[0];

    let mut cipher = XSalsa20::new(key, nonce);
    cipher.simulate_latency(false);
    cipher.encrypt(&mut data).await.unwrap();

    assert_ne!(data[0], original);

    let mut cipher2 = XSalsa20::new(XSalsa20Key::from_bytes([0xbb; 32]), nonce);
    cipher2.simulate_latency(false);
    cipher2.decrypt(&mut data).await.unwrap();

    assert_eq!(data[0], original);
}

/// Test hardware latency simulation
#[tokio::test]
async fn test_hardware_latency() {
    use std::time::Instant;

    let _key = XSalsa20Key::from_bytes([0xdd; 32]);
    let nonce = [0xee; 24];
    let mut data = vec![0; 64];

    // With latency
    let mut cipher = XSalsa20::new(XSalsa20Key::from_bytes([0xdd; 32]), nonce);
    cipher.simulate_latency(true);

    let start = Instant::now();
    cipher.encrypt(&mut data).await.unwrap();
    let duration = start.elapsed();

    // Should take at least 10µs (we use 15µs per block)
    assert!(duration.as_micros() >= 10);

    // Without latency
    let mut cipher2 = XSalsa20::new(XSalsa20Key::from_bytes([0xdd; 32]), nonce);
    cipher2.simulate_latency(false);

    let start = Instant::now();
    cipher2.encrypt(&mut data).await.unwrap();
    let duration = start.elapsed();

    // Should be much faster
    assert!(duration.as_micros() < 1000);
}
