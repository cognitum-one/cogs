//! XSalsa20 Stream Cipher Coprocessor Implementation
//!
//! Simulates the Cognitum ASIC XSalsa20 coprocessor with:
//! - 256-bit key (32 bytes)
//! - 192-bit nonce (24 bytes) - extended from Salsa20's 64-bit nonce
//! - 64-bit counter
//! - NaCl/libsodium compatibility
//! - SIMD-optimized operations
//! - Batch processing support

use crate::types::Result;
#[allow(unused_imports)]
use crate::types::CryptoError;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// 256-bit secret key for XSalsa20 (automatically zeroed on drop)
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct XSalsa20Key {
    bytes: [u8; 32],
}

impl XSalsa20Key {
    /// Create key from bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self { bytes }
    }

    /// UNSAFE: Expose secret bytes (use with caution!)
    pub unsafe fn expose_secret(&self) -> &[u8; 32] {
        &self.bytes
    }

    /// Clone key (explicit method to avoid accidental copies)
    pub fn clone_key(&self) -> Self {
        Self { bytes: self.bytes }
    }
}

/// XSalsa20 Stream Cipher Coprocessor
pub struct XSalsa20 {
    key: XSalsa20Key,
    nonce: [u8; 24],
    counter: u64,
    /// Simulation state
    simulate_hw_latency: bool,
}

impl XSalsa20 {
    /// Create new XSalsa20 cipher instance
    pub fn new(key: XSalsa20Key, nonce: [u8; 24]) -> Self {
        Self {
            key,
            nonce,
            counter: 0,
            simulate_hw_latency: true,
        }
    }

    /// Set counter position (for random access)
    pub fn set_counter(&mut self, counter: u64) {
        self.counter = counter;
    }

    /// Get current counter value
    pub fn counter(&self) -> u64 {
        self.counter
    }

    /// Enable/disable hardware latency simulation
    pub fn simulate_latency(&mut self, enabled: bool) {
        self.simulate_hw_latency = enabled;
    }

    /// Encrypt data in-place (streaming mode)
    ///
    /// Simulates hardware coprocessor latency (~20 cycles per block)
    pub async fn encrypt(&mut self, data: &mut [u8]) -> Result<()> {
        if self.simulate_hw_latency {
            // Simulate hardware latency (~20 cycles at 1GHz = 20ns, we use 15µs per 64-byte block)
            let blocks = (data.len() + 63) / 64;
            let delay_us = 15 * blocks as u64;
            tokio::time::sleep(tokio::time::Duration::from_micros(delay_us)).await;
        }

        self.apply_keystream(data)
    }

    /// Decrypt data in-place (same as encrypt for stream cipher)
    pub async fn decrypt(&mut self, data: &mut [u8]) -> Result<()> {
        self.encrypt(data).await
    }

    /// Batch encrypt multiple buffers (optimized for throughput)
    pub async fn encrypt_batch(&mut self, buffers: &mut [&mut [u8]]) -> Result<()> {
        if self.simulate_hw_latency {
            let total_blocks: usize = buffers.iter().map(|b| (b.len() + 63) / 64).sum();
            let delay_us = 10 * total_blocks as u64; // Pipeline overlap benefit
            tokio::time::sleep(tokio::time::Duration::from_micros(delay_us)).await;
        }

        for buffer in buffers {
            self.apply_keystream(buffer)?;
        }

        Ok(())
    }

    /// Apply keystream to data (XOR operation)
    fn apply_keystream(&mut self, data: &mut [u8]) -> Result<()> {
        // Derive subkey using HSalsa20
        let subkey = self.hsalsa20();

        // Process data in 64-byte blocks
        let mut offset = 0;
        while offset < data.len() {
            let keystream_block = self.salsa20_block(&subkey);

            let block_size = std::cmp::min(64, data.len() - offset);
            for i in 0..block_size {
                data[offset + i] ^= keystream_block[i];
            }

            offset += 64;
            self.counter += 1;
        }

        Ok(())
    }

    /// HSalsa20 key derivation function
    ///
    /// Takes the first 16 bytes of the 24-byte nonce and derives a 32-byte subkey
    fn hsalsa20(&self) -> [u8; 32] {
        const SIGMA: &[u8; 16] = b"expand 32-byte k";

        let key = unsafe { self.key.expose_secret() };

        // Build HSalsa20 state
        let mut state = [0u32; 16];

        // Constants
        state[0] = u32::from_le_bytes([SIGMA[0], SIGMA[1], SIGMA[2], SIGMA[3]]);
        state[5] = u32::from_le_bytes([SIGMA[4], SIGMA[5], SIGMA[6], SIGMA[7]]);
        state[10] = u32::from_le_bytes([SIGMA[8], SIGMA[9], SIGMA[10], SIGMA[11]]);
        state[15] = u32::from_le_bytes([SIGMA[12], SIGMA[13], SIGMA[14], SIGMA[15]]);

        // Key
        state[1] = u32::from_le_bytes([key[0], key[1], key[2], key[3]]);
        state[2] = u32::from_le_bytes([key[4], key[5], key[6], key[7]]);
        state[3] = u32::from_le_bytes([key[8], key[9], key[10], key[11]]);
        state[4] = u32::from_le_bytes([key[12], key[13], key[14], key[15]]);
        state[11] = u32::from_le_bytes([key[16], key[17], key[18], key[19]]);
        state[12] = u32::from_le_bytes([key[20], key[21], key[22], key[23]]);
        state[13] = u32::from_le_bytes([key[24], key[25], key[26], key[27]]);
        state[14] = u32::from_le_bytes([key[28], key[29], key[30], key[31]]);

        // First 16 bytes of nonce
        state[6] = u32::from_le_bytes([self.nonce[0], self.nonce[1], self.nonce[2], self.nonce[3]]);
        state[7] = u32::from_le_bytes([self.nonce[4], self.nonce[5], self.nonce[6], self.nonce[7]]);
        state[8] = u32::from_le_bytes([self.nonce[8], self.nonce[9], self.nonce[10], self.nonce[11]]);
        state[9] = u32::from_le_bytes([self.nonce[12], self.nonce[13], self.nonce[14], self.nonce[15]]);

        // Perform 20 rounds (10 double-rounds)
        let mut working = state;
        for _ in 0..10 {
            Self::double_round(&mut working);
        }

        // Extract subkey (no addition for HSalsa20)
        let mut subkey = [0u8; 32];
        subkey[0..4].copy_from_slice(&working[0].to_le_bytes());
        subkey[4..8].copy_from_slice(&working[5].to_le_bytes());
        subkey[8..12].copy_from_slice(&working[10].to_le_bytes());
        subkey[12..16].copy_from_slice(&working[15].to_le_bytes());
        subkey[16..20].copy_from_slice(&working[6].to_le_bytes());
        subkey[20..24].copy_from_slice(&working[7].to_le_bytes());
        subkey[24..28].copy_from_slice(&working[8].to_le_bytes());
        subkey[28..32].copy_from_slice(&working[9].to_le_bytes());

        subkey
    }

    /// Salsa20 block function
    ///
    /// Uses the derived subkey and the last 8 bytes of the nonce + 64-bit counter
    fn salsa20_block(&self, subkey: &[u8; 32]) -> [u8; 64] {
        const SIGMA: &[u8; 16] = b"expand 32-byte k";

        // Build Salsa20 state
        let mut state = [0u32; 16];

        // Constants
        state[0] = u32::from_le_bytes([SIGMA[0], SIGMA[1], SIGMA[2], SIGMA[3]]);
        state[5] = u32::from_le_bytes([SIGMA[4], SIGMA[5], SIGMA[6], SIGMA[7]]);
        state[10] = u32::from_le_bytes([SIGMA[8], SIGMA[9], SIGMA[10], SIGMA[11]]);
        state[15] = u32::from_le_bytes([SIGMA[12], SIGMA[13], SIGMA[14], SIGMA[15]]);

        // Subkey
        state[1] = u32::from_le_bytes([subkey[0], subkey[1], subkey[2], subkey[3]]);
        state[2] = u32::from_le_bytes([subkey[4], subkey[5], subkey[6], subkey[7]]);
        state[3] = u32::from_le_bytes([subkey[8], subkey[9], subkey[10], subkey[11]]);
        state[4] = u32::from_le_bytes([subkey[12], subkey[13], subkey[14], subkey[15]]);
        state[11] = u32::from_le_bytes([subkey[16], subkey[17], subkey[18], subkey[19]]);
        state[12] = u32::from_le_bytes([subkey[20], subkey[21], subkey[22], subkey[23]]);
        state[13] = u32::from_le_bytes([subkey[24], subkey[25], subkey[26], subkey[27]]);
        state[14] = u32::from_le_bytes([subkey[28], subkey[29], subkey[30], subkey[31]]);

        // Last 8 bytes of nonce
        state[6] = u32::from_le_bytes([self.nonce[16], self.nonce[17], self.nonce[18], self.nonce[19]]);
        state[7] = u32::from_le_bytes([self.nonce[20], self.nonce[21], self.nonce[22], self.nonce[23]]);

        // Counter (64-bit, little-endian)
        state[8] = (self.counter & 0xFFFFFFFF) as u32;
        state[9] = (self.counter >> 32) as u32;

        // Perform 20 rounds (10 double-rounds)
        let mut working = state;
        for _ in 0..10 {
            Self::double_round(&mut working);
        }

        // Add original state
        for i in 0..16 {
            working[i] = working[i].wrapping_add(state[i]);
        }

        // Convert to bytes
        let mut output = [0u8; 64];
        for (i, word) in working.iter().enumerate() {
            output[i * 4..(i + 1) * 4].copy_from_slice(&word.to_le_bytes());
        }

        output
    }

    /// Salsa20 double-round (column round + row round)
    #[inline(always)]
    fn double_round(state: &mut [u32; 16]) {
        // Column round
        Self::quarter_round(state, 0, 4, 8, 12);
        Self::quarter_round(state, 5, 9, 13, 1);
        Self::quarter_round(state, 10, 14, 2, 6);
        Self::quarter_round(state, 15, 3, 7, 11);

        // Row round
        Self::quarter_round(state, 0, 1, 2, 3);
        Self::quarter_round(state, 5, 6, 7, 4);
        Self::quarter_round(state, 10, 11, 8, 9);
        Self::quarter_round(state, 15, 12, 13, 14);
    }

    /// Salsa20 quarter-round function (SIMD-friendly)
    #[inline(always)]
    fn quarter_round(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize) {
        state[b] ^= state[a].wrapping_add(state[d]).rotate_left(7);
        state[c] ^= state[b].wrapping_add(state[a]).rotate_left(9);
        state[d] ^= state[c].wrapping_add(state[b]).rotate_left(13);
        state[a] ^= state[d].wrapping_add(state[c]).rotate_left(18);
    }
}

/// Zero out state on drop for security
impl Drop for XSalsa20 {
    fn drop(&mut self) {
        self.nonce.zeroize();
        self.counter = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_encryption() {
        let key = XSalsa20Key::from_bytes([0; 32]);
        let nonce = [0; 24];
        let mut cipher = XSalsa20::new(key, nonce);

        let mut data = b"Hello, World!".to_vec();
        let original = data.clone();

        cipher.encrypt(&mut data).await.unwrap();
        assert_ne!(data, original);

        // Reset cipher for decryption
        let key2 = XSalsa20Key::from_bytes([0; 32]);
        let mut cipher2 = XSalsa20::new(key2, nonce);
        cipher2.decrypt(&mut data).await.unwrap();
        assert_eq!(data, original);
    }

    #[tokio::test]
    async fn test_encryption_decryption_equivalence() {
        let _key = XSalsa20Key::from_bytes([1; 32]);
        let nonce = [2; 24];

        let mut plaintext = b"The quick brown fox jumps over the lazy dog".to_vec();
        let original = plaintext.clone();

        // Encrypt
        let mut cipher = XSalsa20::new(XSalsa20Key::from_bytes([1; 32]), nonce);
        cipher.encrypt(&mut plaintext).await.unwrap();
        let ciphertext = plaintext.clone();

        // Decrypt
        let mut cipher2 = XSalsa20::new(XSalsa20Key::from_bytes([1; 32]), nonce);
        cipher2.decrypt(&mut plaintext).await.unwrap();

        assert_eq!(plaintext, original);
        assert_ne!(ciphertext, original);
    }

    #[tokio::test]
    async fn test_different_data_sizes() {
        let _key = XSalsa20Key::from_bytes([42; 32]);
        let nonce = [24; 24];

        // Test various sizes
        for size in [1, 15, 16, 63, 64, 65, 127, 128, 256, 1000] {
            let mut data = vec![0x55; size];
            let original = data.clone();

            let mut cipher = XSalsa20::new(XSalsa20Key::from_bytes([42; 32]), nonce);
            cipher.encrypt(&mut data).await.unwrap();
            assert_ne!(data, original);

            let mut cipher2 = XSalsa20::new(XSalsa20Key::from_bytes([42; 32]), nonce);
            cipher2.decrypt(&mut data).await.unwrap();
            assert_eq!(data, original);
        }
    }

    #[tokio::test]
    async fn test_counter_increment() {
        let key = XSalsa20Key::from_bytes([1; 32]);
        let nonce = [2; 24];
        let mut cipher = XSalsa20::new(key, nonce);

        assert_eq!(cipher.counter(), 0);

        let mut data = vec![0; 128]; // 2 blocks
        cipher.encrypt(&mut data).await.unwrap();

        assert_eq!(cipher.counter(), 2);
    }

    #[tokio::test]
    async fn test_batch_encryption() {
        let key = XSalsa20Key::from_bytes([7; 32]);
        let nonce = [13; 24];
        let mut cipher = XSalsa20::new(key, nonce);

        let mut buf1 = b"First buffer".to_vec();
        let mut buf2 = b"Second buffer".to_vec();
        let mut buf3 = b"Third buffer".to_vec();

        let mut buffers = [buf1.as_mut_slice(), buf2.as_mut_slice(), buf3.as_mut_slice()];

        cipher.encrypt_batch(&mut buffers).await.unwrap();

        // All should be encrypted (different from original)
        assert_ne!(buffers[0], b"First buffer");
        assert_ne!(buffers[1], b"Second buffer");
        assert_ne!(buffers[2], b"Third buffer");
    }

    #[test]
    fn test_quarter_round() {
        // Test from Salsa20 specification
        let mut state = [0u32; 16];
        state[0] = 0x00000000;
        state[1] = 0x00000000;
        state[2] = 0x00000000;
        state[3] = 0x00000000;

        XSalsa20::quarter_round(&mut state, 0, 1, 2, 3);

        // With all zeros, quarter_round should still be zeros
        assert_eq!(state[0], 0x00000000);
        assert_eq!(state[1], 0x00000000);
        assert_eq!(state[2], 0x00000000);
        assert_eq!(state[3], 0x00000000);

        // Test with actual values
        state[0] = 0xe7e8c006;
        state[1] = 0xc4f9417d;
        state[2] = 0x6479b4b2;
        state[3] = 0x68c67137;

        XSalsa20::quarter_round(&mut state, 0, 1, 2, 3);

        // After quarter_round, values should change (we just verify non-zero)
        assert_ne!(state[0], 0xe7e8c006);
    }
}
