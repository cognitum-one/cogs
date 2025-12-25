//! Feature flags for different license tiers

use serde::{Deserialize, Serialize};
use crate::license::LicenseTier;

/// Features that can be enabled/disabled per license
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Feature {
    /// API access
    ApiAccess,

    /// Advanced debugging tools
    AdvancedDebug,

    /// HIPAA compliance features
    HipaaCompliance,

    /// Priority support
    PrioritySupport,

    /// Custom hardware models
    CustomModels,

    /// Distributed simulation
    DistributedSim,

    /// Cloud deployment
    CloudDeploy,

    /// Export to hardware formats
    HardwareExport,

    /// Custom feature (for beta/experimental features)
    CustomFeature(String),
}

impl std::fmt::Display for Feature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiAccess => write!(f, "API Access"),
            Self::AdvancedDebug => write!(f, "Advanced Debugging"),
            Self::HipaaCompliance => write!(f, "HIPAA Compliance"),
            Self::PrioritySupport => write!(f, "Priority Support"),
            Self::CustomModels => write!(f, "Custom Hardware Models"),
            Self::DistributedSim => write!(f, "Distributed Simulation"),
            Self::CloudDeploy => write!(f, "Cloud Deployment"),
            Self::HardwareExport => write!(f, "Hardware Export"),
            Self::CustomFeature(name) => write!(f, "Custom: {}", name),
        }
    }
}

/// Checker for feature availability
pub struct FeatureChecker;

impl FeatureChecker {
    /// Create a new feature checker
    pub fn new() -> Self {
        Self
    }

    /// Get default features for a tier
    pub fn features_for_tier(&self, tier: LicenseTier) -> Vec<Feature> {
        match tier {
            LicenseTier::Free => vec![],
            LicenseTier::Developer => vec![
                Feature::ApiAccess,
                Feature::AdvancedDebug,
            ],
            LicenseTier::Professional => vec![
                Feature::ApiAccess,
                Feature::AdvancedDebug,
                Feature::CustomModels,
                Feature::DistributedSim,
                Feature::CloudDeploy,
            ],
            LicenseTier::Enterprise => vec![
                Feature::ApiAccess,
                Feature::AdvancedDebug,
                Feature::HipaaCompliance,
                Feature::PrioritySupport,
                Feature::CustomModels,
                Feature::DistributedSim,
                Feature::CloudDeploy,
                Feature::HardwareExport,
            ],
        }
    }

    /// Check if a license has a specific feature
    pub fn check(&self, license: &crate::license::License, feature: Feature) -> bool {
        // Check tier defaults
        let tier_features = self.features_for_tier(license.tier);
        if tier_features.contains(&feature) {
            return true;
        }

        // Check custom features
        license.features.contains(&feature)
    }

    /// Get all features for a license (tier defaults + custom)
    pub fn all_features(&self, license: &crate::license::License) -> Vec<Feature> {
        let mut features = self.features_for_tier(license.tier);
        for custom in &license.features {
            if !features.contains(custom) {
                features.push(custom.clone());
            }
        }
        features
    }
}

impl Default for FeatureChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::License;
    use chrono::Utc;

    fn create_test_license(tier: LicenseTier) -> License {
        License {
            key: "test_key".to_string(),
            tier,
            organization: "Test Org".to_string(),
            email: "test@example.com".to_string(),
            max_tiles: tier.max_tiles(),
            max_simulations_per_month: tier.max_simulations_per_month(),
            features: vec![],
            valid_until: Utc::now() + chrono::Duration::days(365),
            issued_at: Utc::now(),
            signature: vec![],
            metadata: Default::default(),
        }
    }

    #[test]
    fn test_free_tier_features() {
        let checker = FeatureChecker::new();
        let features = checker.features_for_tier(LicenseTier::Free);
        assert!(features.is_empty());
    }

    #[test]
    fn test_developer_tier_features() {
        let checker = FeatureChecker::new();
        let features = checker.features_for_tier(LicenseTier::Developer);
        assert!(features.contains(&Feature::ApiAccess));
        assert!(features.contains(&Feature::AdvancedDebug));
        assert!(!features.contains(&Feature::HipaaCompliance));
    }

    #[test]
    fn test_enterprise_has_all_features() {
        let checker = FeatureChecker::new();
        let features = checker.features_for_tier(LicenseTier::Enterprise);
        assert!(features.contains(&Feature::ApiAccess));
        assert!(features.contains(&Feature::HipaaCompliance));
        assert!(features.contains(&Feature::PrioritySupport));
    }

    #[test]
    fn test_custom_features() {
        let checker = FeatureChecker::new();
        let mut license = create_test_license(LicenseTier::Developer);
        license.features.push(Feature::CustomFeature("beta_access".to_string()));

        assert!(checker.check(&license, Feature::ApiAccess));
        assert!(checker.check(&license, Feature::CustomFeature("beta_access".to_string())));
        assert!(!checker.check(&license, Feature::HipaaCompliance));
    }
}
