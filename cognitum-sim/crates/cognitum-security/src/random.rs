//! Cryptographically secure random number generation

use crate::error::CryptoError;
use rand::RngCore;
use rand_core::OsRng;

/// Cryptographically secure random number generator
///
/// Uses the operating system's CSPRNG for generating cryptographically
/// secure random bytes. This is suitable for generating keys, nonces,
/// salts, and other security-critical random data.
pub struct SecureRandom {
    _private: (),
}

impl SecureRandom {
    /// Create a new SecureRandom instance
    pub fn new() -> Self {
        Self { _private: () }
    }

    /// Fill a buffer with cryptographically secure random bytes
    ///
    /// # Arguments
    ///
    /// * `dest` - The buffer to fill with random bytes
    ///
    /// # Errors
    ///
    /// Returns `CryptoError::RandomGenerationFailed` if the OS RNG fails
    pub fn fill(&self, dest: &mut [u8]) -> Result<(), CryptoError> {
        OsRng
            .try_fill_bytes(dest)
            .map_err(|e| CryptoError::RandomGenerationFailed(e.to_string()))
    }

    /// Generate a vector of cryptographically secure random bytes
    ///
    /// # Arguments
    ///
    /// * `len` - The number of random bytes to generate
    ///
    /// # Errors
    ///
    /// Returns `CryptoError::RandomGenerationFailed` if the OS RNG fails
    pub fn generate(&self, len: usize) -> Result<Vec<u8>, CryptoError> {
        let mut bytes = vec![0u8; len];
        self.fill(&mut bytes)?;
        Ok(bytes)
    }

    /// Generate a random nonce for AES-GCM (96 bits / 12 bytes)
    pub fn generate_nonce(&self) -> Result<[u8; 12], CryptoError> {
        let mut nonce = [0u8; 12];
        self.fill(&mut nonce)?;
        Ok(nonce)
    }

    /// Generate a random salt for password hashing (128 bits / 16 bytes)
    pub fn generate_salt(&self) -> Result<[u8; 16], CryptoError> {
        let mut salt = [0u8; 16];
        self.fill(&mut salt)?;
        Ok(salt)
    }

    /// Generate a random API key (256 bits / 32 bytes, base64 encoded)
    pub fn generate_api_key(&self) -> Result<String, CryptoError> {
        let mut key_bytes = [0u8; 32];
        self.fill(&mut key_bytes)?;
        Ok(base64::encode_config(
            &key_bytes,
            base64::URL_SAFE_NO_PAD,
        ))
    }
}

impl Default for SecureRandom {
    fn default() -> Self {
        Self::new()
    }
}

// Helper function to encode bytes as base64 (URL-safe without padding)
mod base64 {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    pub fn encode_config(input: &[u8], _config: UrlSafeNoPad) -> String {
        let mut result = Vec::new();
        let mut i = 0;

        while i + 3 <= input.len() {
            let b1 = input[i];
            let b2 = input[i + 1];
            let b3 = input[i + 2];

            result.push(CHARS[((b1 >> 2) & 0x3F) as usize]);
            result.push(CHARS[(((b1 << 4) | (b2 >> 4)) & 0x3F) as usize]);
            result.push(CHARS[(((b2 << 2) | (b3 >> 6)) & 0x3F) as usize]);
            result.push(CHARS[(b3 & 0x3F) as usize]);

            i += 3;
        }

        // Handle remaining bytes
        if i < input.len() {
            let b1 = input[i];
            result.push(CHARS[((b1 >> 2) & 0x3F) as usize]);

            if i + 1 < input.len() {
                let b2 = input[i + 1];
                result.push(CHARS[(((b1 << 4) | (b2 >> 4)) & 0x3F) as usize]);
                result.push(CHARS[((b2 << 2) & 0x3F) as usize]);
            } else {
                result.push(CHARS[((b1 << 4) & 0x3F) as usize]);
            }
        }

        String::from_utf8(result).expect("Base64 encoding produced invalid UTF-8")
    }

    pub struct UrlSafeNoPad;
    pub const URL_SAFE_NO_PAD: UrlSafeNoPad = UrlSafeNoPad;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_randomness() {
        let rng = SecureRandom::new();
        let mut buf1 = [0u8; 32];
        let mut buf2 = [0u8; 32];

        rng.fill(&mut buf1).unwrap();
        rng.fill(&mut buf2).unwrap();

        // Two random fills should produce different results
        assert_ne!(buf1, buf2);
        // Should not be all zeros
        assert_ne!(buf1, [0u8; 32]);
    }

    #[test]
    fn test_generate_nonce() {
        let rng = SecureRandom::new();
        let nonce1 = rng.generate_nonce().unwrap();
        let nonce2 = rng.generate_nonce().unwrap();

        assert_eq!(nonce1.len(), 12);
        assert_ne!(nonce1, nonce2);
    }

    #[test]
    fn test_generate_salt() {
        let rng = SecureRandom::new();
        let salt1 = rng.generate_salt().unwrap();
        let salt2 = rng.generate_salt().unwrap();

        assert_eq!(salt1.len(), 16);
        assert_ne!(salt1, salt2);
    }

    #[test]
    fn test_generate_api_key() {
        let rng = SecureRandom::new();
        let key1 = rng.generate_api_key().unwrap();
        let key2 = rng.generate_api_key().unwrap();

        assert!(!key1.is_empty());
        assert_ne!(key1, key2);
        // Base64 URL-safe characters only
        assert!(key1.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_'));
    }
}
