//! Error types for the licensing system

use thiserror::Error;

/// License validation and enforcement errors
#[derive(Error, Debug, Clone, PartialEq)]
pub enum LicenseError {
    /// License key format is invalid
    #[error("Invalid license key format")]
    InvalidKey,

    /// License signature verification failed
    #[error("Invalid license signature")]
    InvalidSignature,

    /// License has expired
    #[error("License expired on {expired_at}")]
    Expired { expired_at: String },

    /// License has been revoked
    #[error("License has been revoked")]
    Revoked,

    /// License not found in store
    #[error("License not found: {key}")]
    NotFound { key: String },

    /// Tile limit exceeded for tier
    #[error("Tile limit exceeded: requested {requested}, maximum {max} for tier")]
    TileLimitExceeded { max: u32, requested: u32 },

    /// Quota exceeded for period
    #[error("Quota exceeded: {used}/{limit} {quota_type}")]
    QuotaExceeded {
        quota_type: String,
        limit: u64,
        used: u64,
    },

    /// Feature not available in tier
    #[error("Feature '{feature}' not available in {tier} tier")]
    FeatureNotAvailable { feature: String, tier: String },

    /// IO error during license operations
    #[error("IO error: {0}")]
    Io(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Cryptographic error
    #[error("Cryptographic error: {0}")]
    Crypto(String),

    /// Network error during validation
    #[error("Network error: {0}")]
    Network(String),
}

/// Usage metering errors
#[derive(Error, Debug, Clone, PartialEq)]
pub enum MeterError {
    /// Storage backend error
    #[error("Storage error: {0}")]
    Storage(String),

    /// License not found for metering
    #[error("License not found: {key}")]
    LicenseNotFound { key: String },

    /// Invalid usage event
    #[error("Invalid usage event: {0}")]
    InvalidEvent(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Billing integration errors
#[derive(Error, Debug, Clone, PartialEq)]
pub enum BillingError {
    /// Stripe API error
    #[error("Stripe error: {0}")]
    StripeError(String),

    /// Webhook signature verification failed
    #[error("Invalid webhook signature")]
    InvalidSignature,

    /// Invalid webhook payload
    #[error("Invalid webhook payload: {0}")]
    InvalidPayload(String),

    /// License creation failed after payment
    #[error("Failed to create license: {0}")]
    LicenseCreationFailed(String),

    /// Network error
    #[error("Network error: {0}")]
    Network(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Configuration(String),
}

impl From<std::io::Error> for LicenseError {
    fn from(err: std::io::Error) -> Self {
        LicenseError::Io(err.to_string())
    }
}

impl From<serde_json::Error> for LicenseError {
    fn from(err: serde_json::Error) -> Self {
        LicenseError::Serialization(err.to_string())
    }
}

impl From<serde_json::Error> for MeterError {
    fn from(err: serde_json::Error) -> Self {
        MeterError::Serialization(err.to_string())
    }
}
