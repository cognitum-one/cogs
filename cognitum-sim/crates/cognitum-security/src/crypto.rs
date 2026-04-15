//! Cryptographic primitives for Cognitum security
//!
//! This module provides implementation of core cryptographic operations:
//! - AES-256-GCM authenticated encryption
//! - Ed25519 digital signatures
//! - Argon2id password hashing with timing-attack resistance

use crate::error::CryptoError;
use crate::random::SecureRandom as Rng;
use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Nonce,
};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2, Params, Version,
};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use subtle::ConstantTimeEq;

/// AES-256-GCM cipher for authenticated encryption
///
/// Provides authenticated encryption with associated data (AEAD) using AES-256 in GCM mode.
/// Each encryption operation uses a unique nonce to ensure semantic security.
pub struct AesGcmCipher {
    cipher: Aes256Gcm,
    rng: Rng,
}

impl Drop for AesGcmCipher {
    fn drop(&mut self) {
        // Explicit drop for security clarity
        // Cipher protects key material internally
    }
}

impl AesGcmCipher {
    /// Create a new AES-GCM cipher from a 256-bit key
    ///
    /// # Arguments
    ///
    /// * `key` - A 32-byte (256-bit) encryption key
    ///
    /// # Errors
    ///
    /// Returns `CryptoError::InvalidKeyLength` if the key is not exactly 32 bytes
    pub fn new(key: &[u8]) -> Result<Self, CryptoError> {
        if key.len() != 32 {
            return Err(CryptoError::InvalidKeyLength {
                expected: 32,
                actual: key.len(),
            });
        }

        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

        Ok(Self {
            cipher,
            rng: Rng::new(),
        })
    }

    /// Encrypt plaintext with a unique random nonce
    ///
    /// # Arguments
    ///
    /// * `plaintext` - The data to encrypt
    ///
    /// # Returns
    ///
    /// A tuple of (ciphertext, nonce). The nonce must be stored alongside the ciphertext
    /// for later decryption.
    ///
    /// # Errors
    ///
    /// Returns `CryptoError::EncryptionFailed` if encryption fails
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<(Vec<u8>, [u8; 12]), CryptoError> {
        let nonce_bytes = self.rng.generate_nonce()?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

        Ok((ciphertext, nonce_bytes))
    }

    /// Encrypt plaintext with associated data
    ///
    /// # Arguments
    ///
    /// * `plaintext` - The data to encrypt
    /// * `associated_data` - Additional data to authenticate but not encrypt
    ///
    /// # Returns
    ///
    /// A tuple of (ciphertext, nonce)
    pub fn encrypt_with_aad(
        &self,
        plaintext: &[u8],
        associated_data: &[u8],
    ) -> Result<(Vec<u8>, [u8; 12]), CryptoError> {
        let nonce_bytes = self.rng.generate_nonce()?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let payload = Payload {
            msg: plaintext,
            aad: associated_data,
        };

        let ciphertext = self
            .cipher
            .encrypt(nonce, payload)
            .map_err(|e| CryptoError::EncryptionFailed(e.to_string()))?;

        Ok((ciphertext, nonce_bytes))
    }

    /// Decrypt ciphertext using the provided nonce
    ///
    /// # Arguments
    ///
    /// * `ciphertext` - The encrypted data
    /// * `nonce` - The nonce used during encryption
    ///
    /// # Errors
    ///
    /// Returns `CryptoError::AuthenticationFailed` if the ciphertext has been tampered with
    /// or the wrong key/nonce was used
    pub fn decrypt(&self, ciphertext: &[u8], nonce: &[u8; 12]) -> Result<Vec<u8>, CryptoError> {
        let nonce = Nonce::from_slice(nonce);

        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| CryptoError::AuthenticationFailed)
    }

    /// Decrypt ciphertext with associated data verification
    pub fn decrypt_with_aad(
        &self,
        ciphertext: &[u8],
        nonce: &[u8; 12],
        associated_data: &[u8],
    ) -> Result<Vec<u8>, CryptoError> {
        let nonce = Nonce::from_slice(nonce);

        let payload = Payload {
            msg: ciphertext,
            aad: associated_data,
        };

        self.cipher
            .decrypt(nonce, payload)
            .map_err(|_| CryptoError::AuthenticationFailed)
    }
}

/// Ed25519 digital signature implementation
///
/// Provides deterministic signature generation and verification using Ed25519.
pub struct Ed25519Signer {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
}

impl Drop for Ed25519Signer {
    fn drop(&mut self) {
        // SigningKey implements ZeroizeOnDrop internally
        // VerifyingKey is public key material (no zeroization needed)
    }
}

impl Ed25519Signer {
    /// Generate a new Ed25519 keypair
    pub fn generate() -> Result<Self, CryptoError> {
        let rng = Rng::new();
        let mut seed = [0u8; 32];
        rng.fill(&mut seed)?;

        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();

        Ok(Self {
            signing_key,
            verifying_key,
        })
    }

    /// Create a signer from an existing private key
    pub fn from_bytes(private_key: &[u8; 32]) -> Result<Self, CryptoError> {
        let signing_key = SigningKey::from_bytes(private_key);
        let verifying_key = signing_key.verifying_key();

        Ok(Self {
            signing_key,
            verifying_key,
        })
    }

    /// Sign a message (deterministic)
    ///
    /// Ed25519 signatures are deterministic - the same message will always
    /// produce the same signature with the same key.
    pub fn sign(&self, message: &[u8]) -> Result<Vec<u8>, CryptoError> {
        let signature = self.signing_key.sign(message);
        Ok(signature.to_bytes().to_vec())
    }

    /// Verify a signature
    pub fn verify(&self, message: &[u8], signature: &[u8]) -> Result<bool, CryptoError> {
        if signature.len() != 64 {
            return Ok(false);
        }

        let sig_array: [u8; 64] = signature.try_into().expect("signature is 64 bytes");
        let signature = Signature::from_bytes(&sig_array);

        Ok(self.verifying_key.verify(message, &signature).is_ok())
    }

    /// Get the public key (verifying key)
    pub fn public_key(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    /// Get the private key (signing key)
    ///
    /// WARNING: This exposes the private key. Use with extreme caution.
    pub fn private_key(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }
}

/// Configuration for Argon2 password hashing
#[derive(Debug, Clone, Copy)]
pub struct Argon2Config {
    /// Memory cost in KiB (default: 65536 = 64 MiB)
    pub memory_cost: u32,
    /// Time cost (iterations, default: 3)
    pub time_cost: u32,
    /// Parallelism factor (default: 4)
    pub parallelism: u32,
}

impl Default for Argon2Config {
    fn default() -> Self {
        Self {
            memory_cost: 65536, // 64 MiB
            time_cost: 3,
            parallelism: 4,
        }
    }
}

/// Argon2id password hasher with timing-attack resistance
///
/// Uses Argon2id (hybrid of Argon2i and Argon2d) for password hashing.
/// Verification uses constant-time comparison to prevent timing attacks.
pub struct Argon2Hasher {
    config: Argon2Config,
    rng: Rng,
}

impl Argon2Hasher {
    /// Create a new Argon2 hasher with default configuration
    pub fn new(config: Argon2Config) -> Self {
        Self {
            config,
            rng: Rng::new(),
        }
    }

    /// Hash a password with a random salt
    ///
    /// Returns the password hash in PHC string format (includes algorithm,
    /// parameters, salt, and hash).
    pub fn hash(&self, password: &str) -> Result<String, CryptoError> {
        let params = Params::new(
            self.config.memory_cost,
            self.config.time_cost,
            self.config.parallelism,
            None,
        )
        .map_err(|e| CryptoError::InvalidConfiguration(e.to_string()))?;

        let argon2 = Argon2::new(
            argon2::Algorithm::Argon2id,
            Version::V0x13,
            params,
        );

        let salt_bytes = self.rng.generate_salt()?;
        let salt = SaltString::encode_b64(&salt_bytes)
            .map_err(|e| CryptoError::HashingFailed(e.to_string()))?;

        let hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| CryptoError::HashingFailed(e.to_string()))?;

        Ok(hash.to_string())
    }

    /// Verify a password against a hash using constant-time comparison
    ///
    /// This is resistant to timing attacks as the comparison time is
    /// independent of where the mismatch occurs.
    pub fn verify(&self, password: &str, hash: &str) -> Result<bool, CryptoError> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|_e| CryptoError::VerificationFailed)?;

        let argon2 = Argon2::default();

        Ok(argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .is_ok())
    }

    /// Verify using constant-time byte comparison (for API keys, tokens, etc.)
    pub fn verify_constant_time(expected: &[u8], provided: &[u8]) -> bool {
        expected.ct_eq(provided).into()
    }
}

impl Default for Argon2Hasher {
    fn default() -> Self {
        Self::new(Argon2Config::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aes_gcm_encrypt_decrypt() {
        let key = [0u8; 32];
        let cipher = AesGcmCipher::new(&key).unwrap();
        let plaintext = b"Hello, World!";

        let (ciphertext, nonce) = cipher.encrypt(plaintext).unwrap();
        let decrypted = cipher.decrypt(&ciphertext, &nonce).unwrap();

        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[test]
    fn test_aes_gcm_unique_nonces() {
        let key = [0u8; 32];
        let cipher = AesGcmCipher::new(&key).unwrap();
        let plaintext = b"test data";

        let (ciphertext1, nonce1) = cipher.encrypt(plaintext).unwrap();
        let (ciphertext2, nonce2) = cipher.encrypt(plaintext).unwrap();

        assert_ne!(nonce1, nonce2);
        assert_ne!(ciphertext1, ciphertext2);
    }

    #[test]
    fn test_aes_gcm_tamper_detection() {
        let key = [0u8; 32];
        let cipher = AesGcmCipher::new(&key).unwrap();
        let plaintext = b"sensitive data";

        let (mut ciphertext, nonce) = cipher.encrypt(plaintext).unwrap();

        // Tamper with ciphertext
        ciphertext[0] ^= 0xFF;

        let result = cipher.decrypt(&ciphertext, &nonce);
        assert!(matches!(result, Err(CryptoError::AuthenticationFailed)));
    }

    #[test]
    fn test_ed25519_sign_verify() {
        let signer = Ed25519Signer::generate().unwrap();
        let message = b"Important message";

        let signature = signer.sign(message).unwrap();
        assert!(signer.verify(message, &signature).unwrap());

        // Wrong message should fail
        let wrong_message = b"Different message";
        assert!(!signer.verify(wrong_message, &signature).unwrap());
    }

    #[test]
    fn test_ed25519_deterministic() {
        let signer = Ed25519Signer::generate().unwrap();
        let message = b"test message";

        let sig1 = signer.sign(message).unwrap();
        let sig2 = signer.sign(message).unwrap();

        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_argon2_hash_verify() {
        let hasher = Argon2Hasher::default();
        let password = "my_secure_password_123";

        let hash = hasher.hash(password).unwrap();
        assert!(hasher.verify(password, &hash).unwrap());

        // Wrong password should fail
        assert!(!hasher.verify("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_argon2_different_salts() {
        let hasher = Argon2Hasher::default();
        let password = "password123";

        let hash1 = hasher.hash(password).unwrap();
        let hash2 = hasher.hash(password).unwrap();

        // Same password with different salts produces different hashes
        assert_ne!(hash1, hash2);

        // Both hashes verify correctly
        assert!(hasher.verify(password, &hash1).unwrap());
        assert!(hasher.verify(password, &hash2).unwrap());
    }

    #[test]
    fn test_constant_time_comparison() {
        let correct = b"secret_key_12345";
        let wrong = b"secret_key_99999";

        assert!(Argon2Hasher::verify_constant_time(correct, correct));
        assert!(!Argon2Hasher::verify_constant_time(correct, wrong));
    }
}
