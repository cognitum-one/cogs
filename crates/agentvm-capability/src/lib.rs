//! Capability protocol implementation for Agentic VM
//!
//! This crate implements the channel-based capability protocol for secure
//! communication between guest capsules and the capability proxy.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod derive;
pub mod token;
pub mod validate;
pub mod wire;

use agentvm_types::{Capability, CapabilityId, CapabilityType, Quota};

pub use derive::{DeriveError, DeriveRequest};
pub use validate::{Operation, ValidationResult};
pub use wire::{MessageEnvelope, MessageType, ParseError};

/// Validate a capability for use with a specific operation
pub fn validate_capability(
    cap: &Capability,
    operation: &Operation,
    current_time: u64,
) -> ValidationResult {
    // Check if revoked
    if cap.is_revoked() {
        return ValidationResult::Revoked;
    }

    // Check expiry
    if cap.is_expired(current_time) {
        return ValidationResult::Expired;
    }

    // Check quota
    if cap.quota.is_exhausted() {
        return ValidationResult::QuotaExhausted;
    }

    // Check scope
    if !cap.scope.permits(operation.target()) {
        return ValidationResult::ScopeViolation;
    }

    // Verify signature (proof)
    if !cap.proof.verify(cap) {
        return ValidationResult::InvalidSignature;
    }

    ValidationResult::Valid
}

#[cfg(test)]
mod tests;
