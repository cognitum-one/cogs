//! Common types for Cognitum coprocessors

use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Result type for cryptographic operations
pub type Result<T> = std::result::Result<T, CryptoError>;

/// Cryptographic operation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CryptoError {
    /// Hardware coprocessor error
    HardwareError,
    /// Operation timeout
    Timeout,
    /// Invalid cryptographic key
    InvalidKey,
    /// Invalid nonce
    InvalidNonce,
    /// Authentication tag verification failed
    AuthenticationFailed,
    /// Output buffer too small
    BufferTooSmall,
    /// Nonce reuse detected
    NonceReused,
    /// Generic operation failure
    OperationFailed,
    /// ECC error (double-bit)
    EccError,
    /// Key slot not available
    KeySlotUnavailable,
    /// PUF error
    PufError,
    /// TRNG health test failure
    TrngHealthFailure,
    /// Invalid input parameters
    InvalidInput,
    /// Invalid slot address
    InvalidSlot(usize),
}

impl fmt::Display for CryptoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::HardwareError => write!(f, "Hardware coprocessor error"),
            Self::Timeout => write!(f, "Operation timeout"),
            Self::InvalidKey => write!(f, "Invalid cryptographic key"),
            Self::InvalidNonce => write!(f, "Invalid nonce"),
            Self::AuthenticationFailed => write!(f, "Authentication tag verification failed"),
            Self::BufferTooSmall => write!(f, "Output buffer too small"),
            Self::NonceReused => write!(f, "Nonce reuse detected"),
            Self::OperationFailed => write!(f, "Cryptographic operation failed"),
            Self::EccError => write!(f, "ECC double-bit error"),
            Self::KeySlotUnavailable => write!(f, "Key slot not available"),
            Self::PufError => write!(f, "PUF operation error"),
            Self::TrngHealthFailure => write!(f, "TRNG health test failure"),
            Self::InvalidInput => write!(f, "Invalid input parameters"),
            Self::InvalidSlot(addr) => write!(f, "Invalid slot address: {}", addr),
        }
    }
}

impl std::error::Error for CryptoError {}

/// 128-bit secret key (automatically zeroed on drop)
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Key128 {
    bytes: [u8; 16],
}

impl Key128 {
    /// Create key from bytes
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self { bytes }
    }

    /// UNSAFE: Expose secret bytes (use with caution!)
    pub unsafe fn expose_secret(&self) -> &[u8; 16] {
        &self.bytes
    }

    /// Clone key (explicit method to avoid accidental copies)
    pub fn clone_key(&self) -> Self {
        Self { bytes: self.bytes }
    }
}

/// 256-bit hash output
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Hash256([u8; 32]);

impl Hash256 {
    /// Create from bytes
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Get hash bytes
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for Hash256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash256({})", hex::encode(&self.0[..8]))
    }
}
