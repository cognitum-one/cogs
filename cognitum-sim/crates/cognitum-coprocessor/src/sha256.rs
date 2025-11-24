//! SHA-256 Coprocessor Implementation
//!
//! Simulates the Cognitum ASIC SHA-256 coprocessor with:
//! - FIPS 180-4 compliant hashing
//! - 3-stage execution pipeline
//! - ~70 cycles per 512-bit block
//! - HMAC-SHA256 support

use crate::types::{CryptoError, Hash256, Result};
use sha2::{Digest, Sha256};

/// SHA-256 Hash Coprocessor
pub struct Sha256Coprocessor {
    hasher: Sha256,
    finalized: bool,
}

impl Sha256Coprocessor {
    /// Create new SHA-256 coprocessor
    pub fn new() -> Self {
        Self {
            hasher: Sha256::new(),
            finalized: false,
        }
    }

    /// Hash data in one shot
    ///
    /// Simulates 3-stage pipeline: PRIME1, PRIME2, COMPUTE
    /// ~70 cycles per 512-bit block
    pub async fn hash(&mut self, data: &[u8]) -> Result<Hash256> {
        // Calculate number of 512-bit blocks
        let blocks = (data.len() + 64) / 64; // Include padding

        // Simulate pipeline latency (~70 cycles per block)
        let latency_us = 50 + (blocks as u64 * 20);
        tokio::time::sleep(tokio::time::Duration::from_micros(latency_us)).await;

        // Perform actual hash
        let mut hasher = Sha256::new();
        hasher.update(data);
        let hash: [u8; 32] = hasher.finalize().into();

        Ok(Hash256::from_bytes(hash))
    }

    /// Update hash with more data (streaming mode)
    pub async fn update(&mut self, data: &[u8]) -> Result<()> {
        if self.finalized {
            return Err(CryptoError::OperationFailed);
        }

        // Simulate processing latency
        let blocks = (data.len() + 63) / 64;
        let latency_us = blocks as u64 * 20;
        tokio::time::sleep(tokio::time::Duration::from_micros(latency_us)).await;

        self.hasher.update(data);
        Ok(())
    }

    /// Finalize hash and return result
    pub async fn finalize(&mut self) -> Result<Hash256> {
        if self.finalized {
            return Err(CryptoError::OperationFailed);
        }

        // Simulate finalization
        tokio::time::sleep(tokio::time::Duration::from_micros(30)).await;

        self.finalized = true;
        let hash: [u8; 32] = self.hasher.clone().finalize().into();

        Ok(Hash256::from_bytes(hash))
    }

    /// HMAC-SHA256 (for key derivation and authentication)
    pub async fn hmac(&mut self, key: &[u8], message: &[u8]) -> Result<Hash256> {
        const BLOCK_SIZE: usize = 64;
        const OPAD: u8 = 0x5C;
        const IPAD: u8 = 0x36;

        // Prepare key
        let mut key_block = [0u8; BLOCK_SIZE];
        if key.len() > BLOCK_SIZE {
            let hash = self.hash(key).await?;
            key_block[..32].copy_from_slice(hash.as_bytes());
        } else {
            key_block[..key.len()].copy_from_slice(key);
        }

        // Inner hash: H(K ⊕ ipad || message)
        let mut inner_input = Vec::with_capacity(BLOCK_SIZE + message.len());
        for &byte in key_block.iter() {
            inner_input.push(byte ^ IPAD);
        }
        inner_input.extend_from_slice(message);
        let inner_hash = self.hash(&inner_input).await?;

        // Outer hash: H(K ⊕ opad || inner_hash)
        let mut outer_input = Vec::with_capacity(BLOCK_SIZE + 32);
        for &byte in key_block.iter() {
            outer_input.push(byte ^ OPAD);
        }
        outer_input.extend_from_slice(inner_hash.as_bytes());
        let outer_hash = self.hash(&outer_input).await?;

        Ok(outer_hash)
    }
}

impl Default for Sha256Coprocessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_hash() {
        let mut sha256 = Sha256Coprocessor::new();
        let result = sha256.hash(b"test").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_streaming_hash() {
        let mut sha256 = Sha256Coprocessor::new();
        sha256.update(b"Hello").await.unwrap();
        sha256.update(b"World").await.unwrap();
        let result = sha256.finalize().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_hmac() {
        let mut sha256 = Sha256Coprocessor::new();
        let result = sha256.hmac(b"key", b"message").await;
        assert!(result.is_ok());
    }
}
