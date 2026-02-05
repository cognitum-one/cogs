//! Core types for Agentic VM
//!
//! This crate provides the foundational types used across all Agentic VM components,
//! including capsule identities, capabilities, budgets, and evidence structures.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod budget;
pub mod capsule;
pub mod capability;
pub mod error;
pub mod evidence;

// Re-exports
pub use budget::{Budget, BudgetVector};
pub use capsule::{CapsuleId, CapsuleIdentity, CapsuleManifest};
pub use capability::{
    Capability, CapabilityId, CapabilityProof, CapabilityScope, CapabilityType, Quota, Rights,
};
pub use error::{AgentVmError, Result};
pub use evidence::{EvidenceBundle, EvidenceLevel, EvidenceStatement};

#[cfg(test)]
mod tests;
