//! GCM (Galois Counter Mode) Authenticated Encryption Coprocessor
//!
//! Simulates the Cognitum ASIC GCM coprocessor with:
//! - Karatsuba GF(2^128) multiplier for GHASH
//! - AES-CTR mode encryption
//! - 128-bit authentication tags
//! - Constant-time tag verification
//! - Additional Authenticated Data (AAD) support
//! - ~90 cycle operation latency

use crate::aes::AesCoprocessor;
use crate::types::{CryptoError, Key128, Result};
use ghash::{GHash, universal_hash::UniversalHash};
use zeroize::Zeroize;

/// GCM Coprocessor for authenticated encryption
pub struct GcmCoprocessor {
    /// AES coprocessor for CTR mode encryption
    aes: AesCoprocessor,
    /// Nonce for this operation (must be unique per key)
    nonce: [u8; 12],
    /// Additional Authenticated Data (not encrypted, but authenticated)
    aad: Vec<u8>,
    /// Track used nonces to prevent reuse
    used_nonces: Vec<[u8; 12]>,
}

impl GcmCoprocessor {
    /// Create new GCM coprocessor
    pub fn new() -> Self {
        Self {
            aes: AesCoprocessor::new(),
            nonce: [0; 12],
            aad: Vec::new(),
            used_nonces: Vec::new(),
        }
    }

    /// Set nonce for the next operation (must be unique!)
    ///
    /// # Security
    /// Never reuse a nonce with the same key - this breaks GCM security!
    pub fn set_nonce(&mut self, nonce: [u8; 12]) -> Result<()> {
        // Check for nonce reuse (critical security issue)
        if self.used_nonces.contains(&nonce) {
            return Err(CryptoError::NonceReused);
        }

        self.nonce = nonce;
        self.used_nonces.push(nonce);
        Ok(())
    }

    /// Set Additional Authenticated Data (AAD)
    ///
    /// AAD is authenticated but not encrypted (e.g., packet headers)
    pub fn set_aad(&mut self, aad: Vec<u8>) {
        self.aad = aad;
    }

    /// Clear nonce history
    ///
    /// # Security Warning
    /// This is primarily for testing. In production, nonce reuse is a critical
    /// security vulnerability - only clear if you're absolutely certain the key
    /// will never be reused with any previously-used nonce.
    pub fn clear_nonce_history(&mut self) {
        self.used_nonces.clear();
    }

    /// Encrypt and authenticate plaintext
    ///
    /// Returns (ciphertext, authentication_tag)
    ///
    /// Simulates ~90 cycle latency for the complete operation
    pub async fn encrypt(&mut self, key: &Key128, plaintext: &[u8]) -> Result<(Vec<u8>, [u8; 16])> {
        // Simulate hardware latency (~90 cycles at 1GHz = 90ns, we use 90µs)
        tokio::time::sleep(tokio::time::Duration::from_micros(90)).await;

        // Step 1: Derive H = AES(K, 0^128) for GHASH
        let h_block = self.aes.encrypt_block(key, &[0u8; 16]).await?;

        // Step 2: Create initial counter block from nonce
        // Per NIST SP 800-38D: J_0 = nonce || 0^31 || 1 (counter=1)
        // Plaintext encryption uses counter values starting at 2 (J_0 + 1)
        let mut counter_block = [0u8; 16];
        counter_block[..12].copy_from_slice(&self.nonce);
        counter_block[12..].copy_from_slice(&[0, 0, 0, 2]); // Counter = 2 (J_0 + 1)

        // Step 3: Encrypt plaintext with AES-CTR
        let ciphertext = self.aes_ctr_encrypt(key, plaintext, &counter_block).await?;

        // Step 4: Compute authentication tag with GHASH
        let tag = self.compute_ghash(&h_block, &self.aad, &ciphertext)?;

        // Step 5: Encrypt J_0 (counter=1) for final authentication tag
        // Per NIST SP 800-38D: T = MSB_t(GHASH XOR E(K, J_0))
        let mut tag_counter = [0u8; 16];
        tag_counter[..12].copy_from_slice(&self.nonce);
        tag_counter[12..].copy_from_slice(&[0, 0, 0, 1]); // Counter = 1 (J_0)

        let encrypted_tag_counter = self.aes.encrypt_block(key, &tag_counter).await?;

        // XOR GHASH output with encrypted counter to get final tag
        let mut final_tag = [0u8; 16];
        for i in 0..16 {
            final_tag[i] = tag[i] ^ encrypted_tag_counter[i];
        }

        Ok((ciphertext, final_tag))
    }

    /// Decrypt and verify authenticated ciphertext
    ///
    /// Returns plaintext if authentication succeeds, error if tag verification fails
    ///
    /// Simulates ~90 cycle latency for the complete operation
    pub async fn decrypt(
        &mut self,
        key: &Key128,
        ciphertext: &[u8],
        tag: &[u8; 16],
    ) -> Result<Vec<u8>> {
        // Simulate hardware latency (~90 cycles at 1GHz = 90ns, we use 90µs)
        tokio::time::sleep(tokio::time::Duration::from_micros(90)).await;

        // Step 1: Derive H = AES(K, 0^128) for GHASH
        let h_block = self.aes.encrypt_block(key, &[0u8; 16]).await?;

        // Step 2: Recompute authentication tag
        let computed_tag_ghash = self.compute_ghash(&h_block, &self.aad, ciphertext)?;

        // Step 3: Encrypt J_0 (counter=1) to get final tag
        // Per NIST SP 800-38D: T = MSB_t(GHASH XOR E(K, J_0))
        let mut tag_counter = [0u8; 16];
        tag_counter[..12].copy_from_slice(&self.nonce);
        tag_counter[12..].copy_from_slice(&[0, 0, 0, 1]); // Counter = 1 (J_0)

        let encrypted_tag_counter = self.aes.encrypt_block(key, &tag_counter).await?;

        // XOR GHASH output with encrypted counter to get expected tag
        let mut expected_tag = [0u8; 16];
        for i in 0..16 {
            expected_tag[i] = computed_tag_ghash[i] ^ encrypted_tag_counter[i];
        }

        // Step 4: Verify tag in constant time (CRITICAL for security!)
        if !verify_tag_constant_time(&expected_tag, tag) {
            return Err(CryptoError::AuthenticationFailed);
        }

        // Step 5: Decrypt ciphertext with AES-CTR (only after tag verification!)
        // Per NIST SP 800-38D: Decryption uses same counter values as encryption (starting at 2)
        let mut counter_block = [0u8; 16];
        counter_block[..12].copy_from_slice(&self.nonce);
        counter_block[12..].copy_from_slice(&[0, 0, 0, 2]); // Counter = 2 (J_0 + 1)

        let plaintext = self.aes_ctr_encrypt(key, ciphertext, &counter_block).await?;

        Ok(plaintext)
    }

    /// AES-CTR mode encryption (also used for decryption since CTR is symmetric)
    async fn aes_ctr_encrypt(
        &mut self,
        key: &Key128,
        data: &[u8],
        initial_counter: &[u8; 16],
    ) -> Result<Vec<u8>> {
        let mut result = Vec::with_capacity(data.len());
        let mut counter = *initial_counter;

        // Process data in 16-byte blocks
        for chunk in data.chunks(16) {
            // Encrypt counter block
            let keystream = self.aes.encrypt_block(key, &counter).await?;

            // XOR with plaintext/ciphertext
            for (i, &byte) in chunk.iter().enumerate() {
                result.push(byte ^ keystream[i]);
            }

            // Increment counter (big-endian, last 4 bytes)
            increment_counter(&mut counter);
        }

        Ok(result)
    }

    /// Compute GHASH authentication tag
    ///
    /// GHASH(H, A, C) where:
    /// - H is the hash key (AES(K, 0^128))
    /// - A is the AAD
    /// - C is the ciphertext
    fn compute_ghash(&self, h: &[u8; 16], aad: &[u8], ciphertext: &[u8]) -> Result<[u8; 16]> {
        use ghash::Key;

        // Initialize GHASH with H as the key (init_block = 0 for standard GCM)
        let key = Key::from_slice(h);
        let mut ghash = GHash::new_with_init_block(key, 0);

        // Process AAD (padded to 16-byte blocks)
        let aad_padded = pad_to_block(aad);
        for chunk in aad_padded.chunks(16) {
            ghash.update_padded(chunk);
        }

        // Process ciphertext (padded to 16-byte blocks)
        let ct_padded = pad_to_block(ciphertext);
        for chunk in ct_padded.chunks(16) {
            ghash.update_padded(chunk);
        }

        // Append lengths: len(A) || len(C) in bits (as 64-bit big-endian)
        let mut lengths = [0u8; 16];
        let aad_bits = (aad.len() * 8) as u64;
        let ct_bits = (ciphertext.len() * 8) as u64;

        lengths[0..8].copy_from_slice(&aad_bits.to_be_bytes());
        lengths[8..16].copy_from_slice(&ct_bits.to_be_bytes());

        ghash.update_padded(&lengths);

        // Finalize GHASH
        let tag = ghash.finalize();
        Ok(tag.into())
    }
}

impl Default for GcmCoprocessor {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for GcmCoprocessor {
    fn drop(&mut self) {
        // Zeroize sensitive data
        self.nonce.zeroize();
        self.aad.zeroize();
    }
}

/// Verify authentication tag in constant time (critical for security!)
///
/// Uses subtle crate to prevent timing attacks
fn verify_tag_constant_time(computed: &[u8; 16], provided: &[u8; 16]) -> bool {
    use subtle::ConstantTimeEq;
    computed.ct_eq(provided).into()
}

/// Increment counter block (big-endian, last 4 bytes)
fn increment_counter(counter: &mut [u8; 16]) {
    for i in (12..16).rev() {
        counter[i] = counter[i].wrapping_add(1);
        if counter[i] != 0 {
            break; // No overflow
        }
    }
}

/// Pad data to 16-byte blocks with zeros
fn pad_to_block(data: &[u8]) -> Vec<u8> {
    let mut padded = data.to_vec();
    let remainder = data.len() % 16;
    if remainder != 0 {
        padded.resize(data.len() + (16 - remainder), 0);
    }
    padded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gcm_basic_encrypt_decrypt() {
        let mut gcm = GcmCoprocessor::new();
        let key = Key128::from_bytes([0x42; 16]);
        let plaintext = b"Hello, Cognitum GCM!";

        gcm.set_nonce([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]).unwrap();

        // Encrypt
        let (ciphertext, tag) = gcm.encrypt(&key, plaintext).await.unwrap();

        // Decrypt
        let decrypted = gcm.decrypt(&key, &ciphertext, &tag).await.unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[tokio::test]
    async fn test_gcm_nonce_reuse_detection() {
        let mut gcm = GcmCoprocessor::new();
        let nonce = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

        assert!(gcm.set_nonce(nonce).is_ok());

        // Attempting to reuse nonce should fail
        assert!(matches!(gcm.set_nonce(nonce), Err(CryptoError::NonceReused)));
    }

    #[tokio::test]
    async fn test_gcm_authentication_failure() {
        let mut gcm = GcmCoprocessor::new();
        let key = Key128::from_bytes([0x42; 16]);
        let plaintext = b"Test data";

        gcm.set_nonce([1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]).unwrap();

        let (ciphertext, _tag) = gcm.encrypt(&key, plaintext).await.unwrap();

        // Use wrong tag
        let wrong_tag = [0xff; 16];

        let result = gcm.decrypt(&key, &ciphertext, &wrong_tag).await;
        assert!(matches!(result, Err(CryptoError::AuthenticationFailed)));
    }

    #[test]
    fn test_constant_time_verification() {
        let tag1 = [0x42; 16];
        let tag2 = [0x42; 16];
        let tag3 = [0x43; 16];

        assert!(verify_tag_constant_time(&tag1, &tag2));
        assert!(!verify_tag_constant_time(&tag1, &tag3));
    }
}
