//! Cognitum Cryptographic and AI Accelerator Coprocessors
//!
//! This crate provides high-performance, async implementations of cryptographic
//! coprocessors for the Cognitum ASIC, including:
//!
//! - **AES-128**: Session key encryption with 128 independent key slots
//! - **SHA-256**: Hash coprocessor with 3-stage pipeline
//! - **TRNG**: True Random Number Generator with NIST compliance
//! - **PUF**: Physical Unclonable Function for chip-unique identity
//! - **GCM**: Galois Counter Mode authenticated encryption
//! - **XSalsa20**: Stream cipher with 192-bit nonce (NaCl compatible)
//! - **Session Key Manager**: HKDF-based key derivation hierarchy
//! - **NEWS**: Neuromorphic Event-driven Weighted Spike coprocessor

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod aes;
pub mod gcm;
pub mod news;
pub mod puf;
pub mod session;
pub mod sha256;
pub mod simd;
pub mod trng;
pub mod types;
pub mod xsalsa20;

// Legacy modules (preserved for compatibility)
pub mod ai;
pub mod crypto;

pub use types::{CryptoError, Result};
