//! PUF (Physical Unclonable Function) Coprocessor Implementation
//!
//! Simulates chip-unique PUF with:
//! - Challenge-response pairs (CRPs)
//! - Noise simulation (5-15% bit errors)
//! - Helper data for error correction
//! - Device key derivation

use crate::types::{CryptoError, Result};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Physical Unclonable Function Coprocessor
pub struct PhysicalUF {
    /// Chip-unique seed (simulates silicon variations)
    chip_seed: u64,
    /// Noise enabled flag
    noise_enabled: bool,
    /// Noise rate (0.0 - 1.0)
    noise_rate: f64,
    /// RNG for noise injection
    rng: StdRng,
    /// Tampered flag
    tampered: bool,
}

impl PhysicalUF {
    /// Create new PUF with chip-unique seed
    pub fn new(chip_seed: u64) -> Self {
        Self {
            chip_seed,
            noise_enabled: false,
            noise_rate: 0.0,
            rng: StdRng::seed_from_u64(chip_seed),
            tampered: false,
        }
    }

    /// Challenge-response operation
    pub async fn challenge_response(&mut self, challenge: u64) -> Result<u64> {
        // Simulate PUF delay
        tokio::time::sleep(tokio::time::Duration::from_micros(10)).await;

        if self.tampered {
            // Tampered PUF gives completely different responses
            return Ok(self.rng.gen());
        }

        // Generate deterministic response from chip_seed and challenge
        let mut hasher = DefaultHasher::new();
        self.chip_seed.hash(&mut hasher);
        challenge.hash(&mut hasher);
        let mut response = hasher.finish();

        // Add noise if enabled
        if self.noise_enabled {
            let noise_bits = (64.0 * self.noise_rate) as u32;
            for _ in 0..noise_bits {
                let bit_pos = self.rng.gen_range(0..64);
                response ^= 1u64 << bit_pos;
            }
        }

        Ok(response)
    }

    /// Enable noise simulation
    pub fn enable_noise(&mut self, enabled: bool, rate: f64) {
        self.noise_enabled = enabled;
        self.noise_rate = rate.clamp(0.0, 1.0);
    }

    /// Generate helper data for error correction
    pub async fn generate_helper_data(&self, response: u64) -> Result<Vec<u8>> {
        // Simulate helper data generation (BCH syndrome)
        let mut helper = Vec::with_capacity(32);

        // Store response hash for reconstruction
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&response.to_le_bytes());
        let hash = hasher.finalize();

        helper.extend_from_slice(&hash[..16]); // ECC syndrome
        helper.extend_from_slice(&response.to_le_bytes()); // Original response

        Ok(helper)
    }

    /// Reconstruct key from noisy response using helper data
    pub async fn reconstruct_key(&self, noisy_response: u64, helper_data: &[u8]) -> Result<u64> {
        if helper_data.len() < 24 {
            return Err(CryptoError::PufError);
        }

        // Extract original response from helper data
        let mut response_bytes = [0u8; 8];
        response_bytes.copy_from_slice(&helper_data[16..24]);
        let original = u64::from_le_bytes(response_bytes);

        // Simple error correction: XOR noisy with original to find errors
        let _error_pattern = noisy_response ^ original;

        // In real implementation, use BCH/Reed-Solomon to correct
        // For simulation, just return original
        Ok(original)
    }

    /// Derive 256-bit device key from PUF
    pub async fn derive_device_key(&mut self) -> Result<Vec<u8>> {
        // Use challenge 0 for device key
        let response = self.challenge_response(0).await?;

        // Hash PUF response to derive key
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(b"DEVICE_KEY");
        hasher.update(&response.to_le_bytes());
        hasher.update(&self.chip_seed.to_le_bytes());

        Ok(hasher.finalize().to_vec())
    }

    /// Get chip ID
    pub async fn get_chip_id(&self) -> Result<u64> {
        Ok(self.chip_seed)
    }

    /// Simulate physical tampering
    pub fn simulate_tamper(&mut self) {
        self.tampered = true;
        self.rng = StdRng::from_entropy(); // Randomize responses
    }

    /// Configure oscillators (simulated)
    pub async fn configure_oscillators(&self, _channels: u8, _vector_bits: u8) -> Result<()> {
        Ok(())
    }

    /// Prepare key transfer data (48-bit pmosi interface)
    pub async fn prepare_key_transfer(&self, key: &[u8]) -> Result<Vec<u8>> {
        // Simulate pmosi interface format
        // pmosi[47:0] = {load, addr[7:0], chk[6:0], data[31:0]}
        let mut transfer = Vec::with_capacity(6); // 48 bits = 6 bytes

        transfer.push(0x01); // load flag
        transfer.push(0x00); // address
        transfer.extend_from_slice(&key[..4]); // First 4 bytes of key

        Ok(transfer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_challenge_response() {
        let mut puf = PhysicalUF::new(42);
        let r1 = puf.challenge_response(0x123).await.unwrap();
        let r2 = puf.challenge_response(0x123).await.unwrap();
        assert_eq!(r1, r2); // Consistency
    }

    #[tokio::test]
    async fn test_uniqueness() {
        let mut puf1 = PhysicalUF::new(42);
        let mut puf2 = PhysicalUF::new(43);

        let r1 = puf1.challenge_response(0x123).await.unwrap();
        let r2 = puf2.challenge_response(0x123).await.unwrap();

        assert_ne!(r1, r2); // Different chips
    }

    #[tokio::test]
    async fn test_device_key_derivation() {
        let mut puf = PhysicalUF::new(42);
        let key = puf.derive_device_key().await.unwrap();
        assert_eq!(key.len(), 32); // 256 bits
    }
}
