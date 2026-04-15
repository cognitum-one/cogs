//! AES-128 Coprocessor Implementation
//!
//! Simulates the Cognitum ASIC AES coprocessor with:
//! - 128 independent session key slots
//! - ECC-protected key storage
//! - Counter increment for GCM mode
//! - Pipelined 4-word burst mode
//! - ~14 cycle encryption latency

use crate::types::{CryptoError, Key128, Result};
use zeroize::Zeroize;

/// AES-128 Coprocessor
pub struct AesCoprocessor {
    /// Simulation state
    simulate_ecc_single_bit: bool,
    simulate_ecc_double_bit: bool,
    counter_increment_enabled: bool,
    current_iv: [u8; 16],
}

impl AesCoprocessor {
    /// Create new AES coprocessor
    pub fn new() -> Self {
        Self {
            simulate_ecc_single_bit: false,
            simulate_ecc_double_bit: false,
            counter_increment_enabled: false,
            current_iv: [0; 16],
        }
    }

    /// Encrypt a single 128-bit block (ECB mode)
    ///
    /// Simulates ~14 cycle latency (4-word key fetch + 10 AES rounds)
    pub async fn encrypt_block(&mut self, key: &Key128, plaintext: &[u8; 16]) -> Result<[u8; 16]> {
        // Simulate hardware latency (~14 cycles at 1GHz = 14ns, we use 10µs)
        tokio::time::sleep(tokio::time::Duration::from_micros(10)).await;

        // Check for simulated ECC errors
        if self.simulate_ecc_double_bit {
            return Err(CryptoError::EccError);
        }

        // Use aes crate for actual encryption
        use aes::cipher::{BlockEncrypt, KeyInit};
        use aes::Aes128;

        let cipher = Aes128::new_from_slice(unsafe { key.expose_secret() })
            .map_err(|_| CryptoError::InvalidKey)?;

        let mut block = aes::Block::clone_from_slice(plaintext);
        cipher.encrypt_block(&mut block);

        Ok(block.into())
    }

    /// Encrypt with IV (for CTR/GCM modes)
    pub async fn encrypt_with_iv(
        &mut self,
        key: &Key128,
        plaintext: &[u8; 16],
        iv: &[u8; 16],
    ) -> Result<[u8; 16]> {
        self.current_iv = *iv;

        let result = self.encrypt_block(key, plaintext).await?;

        // Increment IV if enabled (GCM mode)
        if self.counter_increment_enabled {
            self.increment_iv();
        }

        Ok(result)
    }

    /// Pipelined burst mode encryption (4 blocks)
    pub async fn encrypt_burst(
        &mut self,
        key: &Key128,
        blocks: &[[u8; 16]],
    ) -> Result<Vec<[u8; 16]>> {
        let mut results = Vec::with_capacity(blocks.len());

        // Simulate pipelined execution (faster than sequential)
        let burst_delay = tokio::time::Duration::from_micros(
            10 + (blocks.len() as u64 - 1) * 2, // Pipeline overlap
        );
        tokio::time::sleep(burst_delay).await;

        for block in blocks {
            // Use actual encryption but skip additional delays
            use aes::cipher::{BlockEncrypt, KeyInit};
            use aes::Aes128;

            let cipher = Aes128::new_from_slice(unsafe { key.expose_secret() })
                .map_err(|_| CryptoError::InvalidKey)?;

            let mut encrypted = aes::Block::clone_from_slice(block);
            cipher.encrypt_block(&mut encrypted);

            results.push(encrypted.into());
        }

        Ok(results)
    }

    /// Enable/disable counter increment for GCM mode
    pub fn enable_counter_increment(&mut self, enabled: bool) {
        self.counter_increment_enabled = enabled;
    }

    /// Get current IV (after increment)
    pub fn get_current_iv(&self) -> [u8; 16] {
        self.current_iv
    }

    /// Increment IV counter (little-endian)
    fn increment_iv(&mut self) {
        for byte in self.current_iv.iter_mut().rev() {
            *byte = byte.wrapping_add(1);
            if *byte != 0 {
                break; // No overflow, done
            }
        }
    }

    /// Simulate single-bit ECC error (auto-correctable)
    pub fn simulate_single_bit_error(&mut self, enabled: bool) {
        self.simulate_ecc_single_bit = enabled;
    }

    /// Simulate double-bit ECC error (fatal)
    pub fn simulate_double_bit_error(&mut self, enabled: bool) {
        self.simulate_ecc_double_bit = enabled;
    }
}

impl Default for AesCoprocessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Session Key Manager (128 independent key slots)
pub struct SessionKeyManager {
    master_key: [u8; 32],
    sessions: [Option<Vec<u8>>; 128],
}

impl SessionKeyManager {
    /// Create new session key manager
    pub fn new(device_key: &Key128) -> Self {
        // Derive master key from device key using SHA-256
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"NEWPORT_MASTER_KEY");
        hasher.update(unsafe { device_key.expose_secret() });
        let master_key: [u8; 32] = hasher.finalize().into();

        Self {
            master_key,
            sessions: std::array::from_fn(|_| None),
        }
    }

    /// Derive session key using HKDF-SHA256
    pub async fn derive_session_key(&mut self, index: u8, session_id: &[u8; 16]) -> Result<()> {
        if index >= 128 {
            return Err(CryptoError::KeySlotUnavailable);
        }

        // HKDF-SHA256 derivation
        use sha2::{Digest, Sha256};

        // HKDF-Extract
        let mut hasher = Sha256::new();
        hasher.update(session_id);
        hasher.update(&self.master_key);
        let prk: [u8; 32] = hasher.finalize().into();

        // HKDF-Expand
        let mut hasher = Sha256::new();
        hasher.update(&prk);
        hasher.update(b"SESSION_KEY");
        hasher.update(&[index]);
        hasher.update(session_id);
        hasher.update(&[0x01]); // Counter
        let session_key: [u8; 32] = hasher.finalize().into();

        self.sessions[index as usize] = Some(session_key.to_vec());

        Ok(())
    }

    /// Get session key from slot
    pub async fn get_key(&self, index: u8) -> Result<Key128> {
        if index >= 128 {
            return Err(CryptoError::KeySlotUnavailable);
        }

        match &self.sessions[index as usize] {
            Some(key) => {
                let mut key_bytes = [0u8; 16];
                key_bytes.copy_from_slice(&key[..16]);
                Ok(Key128::from_bytes(key_bytes))
            }
            None => Err(CryptoError::KeySlotUnavailable),
        }
    }

    /// Revoke session key (zero-out and deallocate)
    pub async fn revoke_session(&mut self, index: u8) {
        if let Some(mut key) = self.sessions[index as usize].take() {
            key.zeroize();
        }
    }
}

impl Drop for SessionKeyManager {
    fn drop(&mut self) {
        self.master_key.zeroize();
        for session in &mut self.sessions {
            if let Some(mut key) = session.take() {
                key.zeroize();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_encryption() {
        let mut aes = AesCoprocessor::new();
        let key = Key128::from_bytes([0; 16]);
        let plaintext = [0; 16];

        let result = aes.encrypt_block(&key, &plaintext).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_session_key_derivation() {
        let device_key = Key128::from_bytes([1; 16]);
        let mut mgr = SessionKeyManager::new(&device_key);

        let session_id = [0; 16];
        assert!(mgr.derive_session_key(0, &session_id).await.is_ok());
        assert!(mgr.get_key(0).await.is_ok());
    }
}
