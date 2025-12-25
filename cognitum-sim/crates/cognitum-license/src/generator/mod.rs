//! License generation and signing

use crate::{License, LicenseError, LicenseRequest, LicenseTier};
use chrono::{Utc, Duration};
use mockall::automock;

pub mod ed25519;

pub use ed25519::Ed25519Generator;

/// Trait for license generation
#[automock]
pub trait LicenseGenerator: Send + Sync {
    /// Generate new license key for tier
    fn generate(&self, request: LicenseRequest) -> Result<License, LicenseError>;

    /// Sign license with private key
    fn sign(&self, license: &mut License) -> Result<(), LicenseError>;

    /// Revoke existing license
    fn revoke(&self, key: &str) -> Result<(), LicenseError>;
}

/// Generate a unique license key
pub fn generate_license_key(tier: LicenseTier) -> String {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    let random: String = (0..16)
        .map(|_| format!("{:02x}", rng.gen::<u8>()))
        .collect();

    // Simple checksum (in production, use proper checksumming)
    let checksum: u32 = random.bytes().map(|b| b as u32).sum();
    let checksum_str = format!("{:08x}", checksum);

    format!("lic_{}_{}_{}",
        tier.tier_code(),
        random,
        &checksum_str[..6]
    )
}

/// Calculate license expiration date
pub fn calculate_expiration(duration_months: u32) -> chrono::DateTime<Utc> {
    Utc::now() + Duration::days((duration_months * 30) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_unique_keys() {
        let key1 = generate_license_key(LicenseTier::Developer);
        let key2 = generate_license_key(LicenseTier::Developer);

        assert_ne!(key1, key2);
        assert!(key1.starts_with("lic_dev_"));
        assert!(key2.starts_with("lic_dev_"));
    }

    #[test]
    fn test_key_contains_tier() {
        assert!(generate_license_key(LicenseTier::Free).contains("free"));
        assert!(generate_license_key(LicenseTier::Developer).contains("dev"));
        assert!(generate_license_key(LicenseTier::Professional).contains("pro"));
        assert!(generate_license_key(LicenseTier::Enterprise).contains("ent"));
    }

    #[test]
    fn test_expiration_calculation() {
        let expiry = calculate_expiration(12);
        let expected = Utc::now() + Duration::days(360);

        let diff = (expiry - expected).num_days().abs();
        assert!(diff <= 1); // Within 1 day tolerance
    }
}
