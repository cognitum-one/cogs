//! Cryptographic coprocessor

use cognitum_core::Result;

/// Crypto coprocessor
pub struct CryptoCoprocessor;

impl CryptoCoprocessor {
    /// Create a new crypto coprocessor
    pub fn new() -> Self {
        Self
    }

    /// Perform AES encryption
    pub fn aes_encrypt(&self, data: &[u8], _key: &[u8]) -> Result<Vec<u8>> {
        // TODO: Implement AES
        Ok(data.to_vec())
    }
}

impl Default for CryptoCoprocessor {
    fn default() -> Self {
        Self::new()
    }
}
