//! Unit tests for RBAC implementation
//!
//! These tests verify the internal workings of the RBAC system
//! including permission logic, role management, and tier mappings

use cognitum::auth::{Permission, RoleManager, Tier};

#[cfg(test)]
mod permission_tests {
    use super::*;

    #[test]
    fn test_permission_display() {
        assert_eq!(format!("{}", Permission::SimulatorRead), "simulator:read");
        assert_eq!(format!("{}", Permission::SimulatorExecute), "simulator:execute");
        assert_eq!(format!("{}", Permission::SimulatorConfigure), "simulator:configure");
        assert_eq!(format!("{}", Permission::MemoryRead), "memory:read");
        assert_eq!(format!("{}", Permission::MemoryWrite), "memory:write");
        assert_eq!(format!("{}", Permission::AdminFull), "admin:full");
        assert_eq!(format!("{}", Permission::AuditRead), "audit:read");
    }

    #[test]
    fn test_permission_equality() {
        assert_eq!(Permission::SimulatorRead, Permission::SimulatorRead);
        assert_ne!(Permission::SimulatorRead, Permission::SimulatorExecute);
    }
}

#[cfg(test)]
mod tier_tests {
    use super::*;

    #[test]
    fn test_free_tier_permissions() {
        let perms = Tier::Free.permissions();

        assert_eq!(perms.len(), 2);
        assert!(perms.contains(&Permission::SimulatorRead));
        assert!(perms.contains(&Permission::SimulatorExecute));

        // Should NOT have advanced permissions
        assert!(!perms.contains(&Permission::SimulatorConfigure));
        assert!(!perms.contains(&Permission::MemoryRead));
        assert!(!perms.contains(&Permission::MemoryWrite));
        assert!(!perms.contains(&Permission::AdminFull));
    }

    #[test]
    fn test_developer_tier_permissions() {
        let perms = Tier::Developer.permissions();

        assert_eq!(perms.len(), 4);

        // Should have Free tier permissions
        assert!(perms.contains(&Permission::SimulatorRead));
        assert!(perms.contains(&Permission::SimulatorExecute));

        // Plus Developer-specific permissions
        assert!(perms.contains(&Permission::SimulatorConfigure));
        assert!(perms.contains(&Permission::MemoryRead));

        // But NOT Professional/Enterprise permissions
        assert!(!perms.contains(&Permission::MemoryWrite));
        assert!(!perms.contains(&Permission::AdminFull));
    }

    #[test]
    fn test_professional_tier_permissions() {
        let perms = Tier::Professional.permissions();

        assert_eq!(perms.len(), 5);

        // Should have all Developer permissions
        assert!(perms.contains(&Permission::SimulatorRead));
        assert!(perms.contains(&Permission::SimulatorExecute));
        assert!(perms.contains(&Permission::SimulatorConfigure));
        assert!(perms.contains(&Permission::MemoryRead));

        // Plus Professional-specific permission
        assert!(perms.contains(&Permission::MemoryWrite));

        // But NOT Enterprise permissions
        assert!(!perms.contains(&Permission::AdminFull));
        assert!(!perms.contains(&Permission::AuditRead));
    }

    #[test]
    fn test_enterprise_tier_permissions() {
        let perms = Tier::Enterprise.permissions();

        assert_eq!(perms.len(), 7);

        // Should have ALL permissions
        assert!(perms.contains(&Permission::SimulatorRead));
        assert!(perms.contains(&Permission::SimulatorExecute));
        assert!(perms.contains(&Permission::SimulatorConfigure));
        assert!(perms.contains(&Permission::MemoryRead));
        assert!(perms.contains(&Permission::MemoryWrite));
        assert!(perms.contains(&Permission::AdminFull));
        assert!(perms.contains(&Permission::AuditRead));
    }

    #[test]
    fn test_tier_pricing() {
        assert_eq!(Tier::Free.price_per_month(), Some(0));
        assert_eq!(Tier::Developer.price_per_month(), Some(99));
        assert_eq!(Tier::Professional.price_per_month(), Some(499));
        assert_eq!(Tier::Enterprise.price_per_month(), None); // Custom pricing
    }

    #[test]
    fn test_tier_from_role() {
        assert_eq!(Tier::from_role("free"), Some(Tier::Free));
        assert_eq!(Tier::from_role("FREE"), Some(Tier::Free));
        assert_eq!(Tier::from_role("developer"), Some(Tier::Developer));
        assert_eq!(Tier::from_role("DEVELOPER"), Some(Tier::Developer));
        assert_eq!(Tier::from_role("professional"), Some(Tier::Professional));
        assert_eq!(Tier::from_role("enterprise"), Some(Tier::Enterprise));
        assert_eq!(Tier::from_role("unknown"), None);
    }

    #[test]
    fn test_tier_features() {
        let free_features = Tier::Free.features();
        assert!(free_features.len() > 0);
        assert!(free_features.contains(&"Basic simulator access"));

        let dev_features = Tier::Developer.features();
        assert!(dev_features.len() > free_features.len());
        assert!(dev_features.contains(&"All Free features"));

        let ent_features = Tier::Enterprise.features();
        assert!(ent_features.len() > dev_features.len());
        assert!(ent_features.contains(&"SLA guarantees"));
    }
}

#[cfg(test)]
mod role_manager_tests {
    use super::*;

    #[test]
    fn test_role_manager_default_roles() {
        let manager = RoleManager::new();

        // Free role
        let free_perms = manager.get_permissions("free");
        assert_eq!(free_perms.len(), 2);
        assert!(free_perms.contains(&Permission::SimulatorRead));

        // Developer role
        let dev_perms = manager.get_permissions("developer");
        assert_eq!(dev_perms.len(), 4);
        assert!(dev_perms.contains(&Permission::SimulatorConfigure));

        // Professional role
        let pro_perms = manager.get_permissions("professional");
        assert_eq!(pro_perms.len(), 5);
        assert!(pro_perms.contains(&Permission::MemoryWrite));

        // Enterprise role
        let ent_perms = manager.get_permissions("enterprise");
        assert_eq!(ent_perms.len(), 7);
        assert!(ent_perms.contains(&Permission::AdminFull));
    }

    #[test]
    fn test_role_manager_case_insensitive() {
        let manager = RoleManager::new();

        let perms_lower = manager.get_permissions("developer");
        let perms_upper = manager.get_permissions("DEVELOPER");
        let perms_mixed = manager.get_permissions("DeVeLoPeR");

        assert_eq!(perms_lower.len(), perms_upper.len());
        assert_eq!(perms_lower.len(), perms_mixed.len());
    }

    #[test]
    fn test_role_manager_unknown_role() {
        let manager = RoleManager::new();

        let perms = manager.get_permissions("unknown_role");
        assert_eq!(perms.len(), 0);
    }

    #[test]
    fn test_role_manager_has_permission() {
        let manager = RoleManager::new();

        // Free tier checks
        assert!(manager.has_permission("free", Permission::SimulatorRead));
        assert!(manager.has_permission("free", Permission::SimulatorExecute));
        assert!(!manager.has_permission("free", Permission::SimulatorConfigure));

        // Developer tier checks
        assert!(manager.has_permission("developer", Permission::SimulatorRead));
        assert!(manager.has_permission("developer", Permission::SimulatorConfigure));
        assert!(manager.has_permission("developer", Permission::MemoryRead));
        assert!(!manager.has_permission("developer", Permission::MemoryWrite));

        // Professional tier checks
        assert!(manager.has_permission("professional", Permission::MemoryWrite));
        assert!(!manager.has_permission("professional", Permission::AdminFull));

        // Enterprise tier checks
        assert!(manager.has_permission("enterprise", Permission::AdminFull));
        assert!(manager.has_permission("enterprise", Permission::AuditRead));
    }

    #[test]
    fn test_role_manager_add_custom_role() {
        let mut manager = RoleManager::new();

        // Add a custom auditor role
        manager.add_role(
            "auditor".to_string(),
            vec![Permission::AuditRead, Permission::SimulatorRead],
        );

        let perms = manager.get_permissions("auditor");
        assert_eq!(perms.len(), 2);
        assert!(perms.contains(&Permission::AuditRead));
        assert!(perms.contains(&Permission::SimulatorRead));
        assert!(!perms.contains(&Permission::AdminFull));

        // Check has_permission works with custom role
        assert!(manager.has_permission("auditor", Permission::AuditRead));
        assert!(!manager.has_permission("auditor", Permission::MemoryWrite));
    }

    #[test]
    fn test_role_manager_support_role() {
        let manager = RoleManager::new();

        let support_perms = manager.get_permissions("support");
        assert!(support_perms.contains(&Permission::SimulatorRead));
        assert!(support_perms.contains(&Permission::MemoryRead));
        assert!(support_perms.contains(&Permission::AuditRead));

        // Support should NOT have write or admin permissions
        assert!(!support_perms.contains(&Permission::MemoryWrite));
        assert!(!support_perms.contains(&Permission::AdminFull));
    }

    #[test]
    fn test_role_manager_admin_role() {
        let manager = RoleManager::new();

        let admin_perms = manager.get_permissions("admin");

        // Admin should have enterprise-level permissions
        assert!(admin_perms.contains(&Permission::AdminFull));
        assert!(admin_perms.contains(&Permission::AuditRead));
        assert_eq!(admin_perms.len(), 7);
    }
}

#[cfg(test)]
mod tier_hierarchy_tests {
    use super::*;

    #[test]
    fn test_hierarchical_permission_inheritance() {
        // Verify that each tier includes all permissions from lower tiers
        let free_perms: std::collections::HashSet<_> = Tier::Free.permissions().into_iter().collect();
        let dev_perms: std::collections::HashSet<_> = Tier::Developer.permissions().into_iter().collect();
        let pro_perms: std::collections::HashSet<_> = Tier::Professional.permissions().into_iter().collect();
        let ent_perms: std::collections::HashSet<_> = Tier::Enterprise.permissions().into_iter().collect();

        // Developer includes all Free permissions
        assert!(free_perms.is_subset(&dev_perms));

        // Professional includes all Developer permissions
        assert!(dev_perms.is_subset(&pro_perms));

        // Enterprise includes all Professional permissions
        assert!(pro_perms.is_subset(&ent_perms));
    }

    #[test]
    fn test_permission_counts_increase_by_tier() {
        let free_count = Tier::Free.permissions().len();
        let dev_count = Tier::Developer.permissions().len();
        let pro_count = Tier::Professional.permissions().len();
        let ent_count = Tier::Enterprise.permissions().len();

        assert!(dev_count > free_count);
        assert!(pro_count > dev_count);
        assert!(ent_count > pro_count);
    }
}
