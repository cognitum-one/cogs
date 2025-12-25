//! Core license types and structures

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use crate::features::Feature;

/// License tiers with different capabilities and pricing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LicenseTier {
    /// Open source tier - limited features
    /// - 32 tiles max
    /// - 1000 simulations/month
    /// - No API access
    Free,

    /// Individual developer - $99/month
    /// - 256 tiles max
    /// - Unlimited simulations
    /// - 10K API requests/month
    Developer,

    /// Team/startup - $499/month
    /// - 1024 tiles max
    /// - Unlimited simulations
    /// - 100K API requests/month
    Professional,

    /// Large organization - custom pricing
    /// - Unlimited tiles
    /// - Unlimited simulations
    /// - Unlimited API requests
    /// - HIPAA compliance
    /// - Priority support
    Enterprise,
}

impl LicenseTier {
    /// Maximum number of tiles allowed for this tier
    pub fn max_tiles(&self) -> u32 {
        match self {
            Self::Free => 32,
            Self::Developer => 256,
            Self::Professional => 1024,
            Self::Enterprise => u32::MAX,
        }
    }

    /// Maximum simulations per month (None = unlimited)
    pub fn max_simulations_per_month(&self) -> Option<u64> {
        match self {
            Self::Free => Some(1000),
            Self::Developer => None,
            Self::Professional => None,
            Self::Enterprise => None,
        }
    }

    /// Maximum API requests per month (None = unlimited or not available)
    pub fn api_requests_per_month(&self) -> Option<u64> {
        match self {
            Self::Free => None, // No API access
            Self::Developer => Some(10_000),
            Self::Professional => Some(100_000),
            Self::Enterprise => None, // Unlimited
        }
    }

    /// Monthly price in USD cents
    pub fn price_cents(&self) -> Option<u64> {
        match self {
            Self::Free => None,
            Self::Developer => Some(9900),  // $99
            Self::Professional => Some(49900),  // $499
            Self::Enterprise => None,  // Custom pricing
        }
    }

    /// Tier identifier for license keys
    pub fn tier_code(&self) -> &'static str {
        match self {
            Self::Free => "free",
            Self::Developer => "dev",
            Self::Professional => "pro",
            Self::Enterprise => "ent",
        }
    }

    /// Parse tier from code
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "free" => Some(Self::Free),
            "dev" => Some(Self::Developer),
            "pro" => Some(Self::Professional),
            "ent" => Some(Self::Enterprise),
            _ => None,
        }
    }
}

impl std::fmt::Display for LicenseTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Free => write!(f, "Free"),
            Self::Developer => write!(f, "Developer"),
            Self::Professional => write!(f, "Professional"),
            Self::Enterprise => write!(f, "Enterprise"),
        }
    }
}

/// A software license for Cognitum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct License {
    /// Unique license key (format: lic_{tier}_{random}_{checksum})
    pub key: String,

    /// License tier
    pub tier: LicenseTier,

    /// Organization name
    pub organization: String,

    /// Contact email
    pub email: String,

    /// Maximum number of tiles
    pub max_tiles: u32,

    /// Maximum simulations per month (None = unlimited)
    pub max_simulations_per_month: Option<u64>,

    /// Additional features enabled
    pub features: Vec<Feature>,

    /// License valid until this date
    pub valid_until: DateTime<Utc>,

    /// License creation date
    pub issued_at: DateTime<Utc>,

    /// Ed25519 signature (64 bytes)
    pub signature: Vec<u8>,

    /// Optional metadata
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

impl License {
    /// Check if license is currently valid (not expired)
    pub fn is_valid(&self) -> bool {
        Utc::now() < self.valid_until
    }

    /// Check if license is expired
    pub fn is_expired(&self) -> bool {
        !self.is_valid()
    }

    /// Days until expiration (negative if expired)
    pub fn days_until_expiry(&self) -> i64 {
        (self.valid_until - Utc::now()).num_days()
    }

    /// Get max tiles for this license
    pub fn max_tiles(&self) -> u32 {
        self.max_tiles
    }

    /// Get bytes to sign/verify (everything except signature)
    pub fn signable_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend(self.key.as_bytes());
        data.extend(self.tier.tier_code().as_bytes());
        data.extend(self.organization.as_bytes());
        data.extend(self.email.as_bytes());
        data.extend(&self.max_tiles.to_le_bytes());
        data.extend(&self.valid_until.timestamp().to_le_bytes());
        data.extend(&self.issued_at.timestamp().to_le_bytes());
        data
    }
}

/// Request to generate a new license
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseRequest {
    /// Desired tier
    pub tier: LicenseTier,

    /// Organization name
    pub organization: String,

    /// Contact email
    pub email: String,

    /// Duration in months
    pub duration_months: u32,

    /// Additional features
    #[serde(default)]
    pub features: Vec<Feature>,

    /// Optional metadata
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, String>,
}

impl Default for LicenseRequest {
    fn default() -> Self {
        Self {
            tier: LicenseTier::Free,
            organization: String::new(),
            email: String::new(),
            duration_months: 12,
            features: Vec::new(),
            metadata: std::collections::HashMap::new(),
        }
    }
}

/// Usage quota for a license
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UsageQuota {
    /// Maximum simulations per month
    pub max_simulations: Option<u64>,

    /// Maximum cycles total
    pub max_cycles: Option<u64>,

    /// Maximum API requests per month
    pub max_api_requests: Option<u64>,

    /// Maximum tiles per simulation
    pub max_tiles: u32,
}

impl UsageQuota {
    /// Create quota from license tier
    pub fn from_tier(tier: LicenseTier) -> Self {
        Self {
            max_simulations: tier.max_simulations_per_month(),
            max_cycles: None,
            max_api_requests: tier.api_requests_per_month(),
            max_tiles: tier.max_tiles(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_limits() {
        assert_eq!(LicenseTier::Free.max_tiles(), 32);
        assert_eq!(LicenseTier::Developer.max_tiles(), 256);
        assert_eq!(LicenseTier::Professional.max_tiles(), 1024);
        assert_eq!(LicenseTier::Enterprise.max_tiles(), u32::MAX);
    }

    #[test]
    fn test_tier_quotas() {
        assert_eq!(LicenseTier::Free.max_simulations_per_month(), Some(1000));
        assert_eq!(LicenseTier::Developer.max_simulations_per_month(), None);
        assert_eq!(LicenseTier::Free.api_requests_per_month(), None);
        assert_eq!(LicenseTier::Developer.api_requests_per_month(), Some(10_000));
    }

    #[test]
    fn test_tier_codes() {
        assert_eq!(LicenseTier::Free.tier_code(), "free");
        assert_eq!(LicenseTier::Developer.tier_code(), "dev");
        assert_eq!(LicenseTier::Professional.tier_code(), "pro");
        assert_eq!(LicenseTier::Enterprise.tier_code(), "ent");
    }

    #[test]
    fn test_tier_from_code() {
        assert_eq!(LicenseTier::from_code("free"), Some(LicenseTier::Free));
        assert_eq!(LicenseTier::from_code("dev"), Some(LicenseTier::Developer));
        assert_eq!(LicenseTier::from_code("invalid"), None);
    }
}
