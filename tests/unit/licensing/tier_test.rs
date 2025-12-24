//! Licensing tier unit tests

#[cfg(test)]
mod tier_tests {
    #[derive(Debug, Clone, PartialEq)]
    pub enum Tier {
        Free,
        Professional,
        Enterprise,
    }

    impl Tier {
        pub fn max_simulations_per_month(&self) -> u32 {
            match self {
                Tier::Free => 100,
                Tier::Professional => 10_000,
                Tier::Enterprise => u32::MAX,
            }
        }

        pub fn max_concurrent_simulations(&self) -> u32 {
            match self {
                Tier::Free => 1,
                Tier::Professional => 10,
                Tier::Enterprise => 100,
            }
        }

        pub fn max_tiles(&self) -> u32 {
            match self {
                Tier::Free => 64,
                Tier::Professional => 512,
                Tier::Enterprise => 4096,
            }
        }

        pub fn has_api_access(&self) -> bool {
            !matches!(self, Tier::Free)
        }

        pub fn has_priority_support(&self) -> bool {
            matches!(self, Tier::Enterprise)
        }
    }

    #[test]
    fn free_tier_has_limited_quotas() {
        let tier = Tier::Free;

        assert_eq!(tier.max_simulations_per_month(), 100);
        assert_eq!(tier.max_concurrent_simulations(), 1);
        assert_eq!(tier.max_tiles(), 64);
        assert!(!tier.has_api_access());
        assert!(!tier.has_priority_support());
    }

    #[test]
    fn professional_tier_has_moderate_quotas() {
        let tier = Tier::Professional;

        assert_eq!(tier.max_simulations_per_month(), 10_000);
        assert_eq!(tier.max_concurrent_simulations(), 10);
        assert_eq!(tier.max_tiles(), 512);
        assert!(tier.has_api_access());
        assert!(!tier.has_priority_support());
    }

    #[test]
    fn enterprise_tier_has_unlimited_quotas() {
        let tier = Tier::Enterprise;

        assert_eq!(tier.max_simulations_per_month(), u32::MAX);
        assert_eq!(tier.max_concurrent_simulations(), 100);
        assert_eq!(tier.max_tiles(), 4096);
        assert!(tier.has_api_access());
        assert!(tier.has_priority_support());
    }

    #[test]
    fn tier_comparison() {
        let free = Tier::Free;
        let pro = Tier::Professional;
        let ent = Tier::Enterprise;

        assert!(free.max_tiles() < pro.max_tiles());
        assert!(pro.max_tiles() < ent.max_tiles());
    }
}
