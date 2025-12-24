//! # Cognitum License System
//!
//! A comprehensive licensing and usage metering system for the Cognitum chip simulator.
//!
//! ## Features
//!
//! - **Ed25519 Cryptographic Signing**: Secure license validation using modern cryptography
//! - **Offline Validation**: Licenses can be validated without network access
//! - **Usage Metering**: Track simulations, cycles, and API requests
//! - **Quota Enforcement**: Tier-based limits on usage
//! - **Billing Integration**: Optional Stripe integration for payments
//! - **Feature Gating**: Control access to features by tier
//!
//! ## License Tiers
//!
//! - **Free**: 32 tiles, 1000 simulations/month
//! - **Developer**: 256 tiles, unlimited simulations, $99/month
//! - **Professional**: 1024 tiles, unlimited simulations, $499/month
//! - **Enterprise**: Unlimited tiles and simulations, custom pricing
//!
//! ## Example
//!
//! ```rust,no_run
//! use cognitum_license::{LicenseValidator, Ed25519Validator};
//! use cognitum_license::store::InMemoryStore;
//! use ed25519_dalek::SigningKey;
//! use std::sync::Arc;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create validator with keys
//! let signing_key = SigningKey::from_bytes(&[1u8; 32]);
//! let public_key = signing_key.verifying_key();
//! let store = Arc::new(InMemoryStore::new());
//! let validator = Ed25519Validator::new(public_key, store);
//!
//! // Now you can validate license keys
//! // let license = validator.validate("lic_dev_example_key")?;
//! # Ok(())
//! # }
//! ```

pub mod errors;
pub mod license;
pub mod features;
pub mod validator;
pub mod generator;
pub mod meter;
pub mod store;

#[cfg(feature = "billing")]
pub mod billing;

// Re-export main types
pub use errors::{LicenseError, MeterError, BillingError};
pub use license::{License, LicenseTier, LicenseRequest, UsageQuota};
pub use features::{Feature, FeatureChecker};
pub use validator::{LicenseValidator, Ed25519Validator};
pub use generator::{LicenseGenerator, Ed25519Generator};
pub use meter::{UsageMeter, UsageEvent, Usage, Period, QuotaResult, InMemoryMeter};
pub use store::{LicenseStore, InMemoryStore};

#[cfg(feature = "billing")]
pub use billing::{BillingClient, StripeBillingClient, WebhookResult, CheckoutSession};

/// Operation types for quota checking
#[derive(Debug, Clone, PartialEq)]
pub enum Operation {
    /// Create a simulation with specified tile count
    CreateSimulation { tiles: u32 },
    /// Run a simulation
    RunSimulation,
    /// Make an API request
    ApiRequest { endpoint: String },
}
