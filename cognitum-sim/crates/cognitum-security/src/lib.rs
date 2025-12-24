//! Cognitum Security Infrastructure
//!
//! This module provides comprehensive cryptographic security for the Cognitum chip v1 platform,
//! including:
//! - AES-GCM encryption with unique nonces
//! - Ed25519 digital signatures
//! - Argon2 password hashing with timing-attack resistance
//! - Hardware Security Module (HSM) integration
//! - Key management with circuit breaker pattern
//! - Cryptographically secure random number generation

pub mod crypto;
pub mod error;
pub mod kms;
pub mod random;

pub use crypto::{AesGcmCipher, Argon2Config, Argon2Hasher, Ed25519Signer};
pub use error::{CryptoError, HsmError, KmsError};
pub use kms::{
    CircuitBreakerConfig, HsmProvider, KeyManagementService, KeyPurpose, MockHsmProvider,
};
pub use random::SecureRandom;

/// Re-export commonly used types
pub type Result<T> = std::result::Result<T, error::CryptoError>;
