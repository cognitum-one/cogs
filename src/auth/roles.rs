//! Role definitions and tier-based permission mappings
//!
//! This module defines the subscription tiers and their associated permissions
//! following the commercialization pricing model.

use super::rbac::Permission;
use std::collections::HashMap;

/// Subscription tiers for Cognitum chip access
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    /// Free tier - Basic simulator access
    Free,
    /// Developer tier - $99/month - Advanced configuration
    Developer,
    /// Professional tier - $499/month - Full memory access
    Professional,
    /// Enterprise tier - Custom pricing - Full administrative access
    Enterprise,
}

impl Tier {
    /// Convert string role name to tier
    pub fn from_role(role: &str) -> Option<Self> {
        match role.to_lowercase().as_str() {
            "free" => Some(Tier::Free),
            "developer" => Some(Tier::Developer),
            "professional" => Some(Tier::Professional),
            "enterprise" => Some(Tier::Enterprise),
            _ => None,
        }
    }

    /// Get all permissions for this tier
    ///
    /// Permissions are hierarchical - higher tiers include all lower tier permissions
    pub fn permissions(&self) -> Vec<Permission> {
        match self {
            Tier::Free => vec![
                Permission::SimulatorRead,
                Permission::SimulatorExecute,
            ],
            Tier::Developer => vec![
                // Inherits from Free
                Permission::SimulatorRead,
                Permission::SimulatorExecute,
                // Additional Developer permissions
                Permission::SimulatorConfigure,
                Permission::MemoryRead,
            ],
            Tier::Professional => vec![
                // Inherits from Developer
                Permission::SimulatorRead,
                Permission::SimulatorExecute,
                Permission::SimulatorConfigure,
                Permission::MemoryRead,
                // Additional Professional permissions
                Permission::MemoryWrite,
            ],
            Tier::Enterprise => vec![
                // Inherits from Professional
                Permission::SimulatorRead,
                Permission::SimulatorExecute,
                Permission::SimulatorConfigure,
                Permission::MemoryRead,
                Permission::MemoryWrite,
                // Additional Enterprise permissions
                Permission::AdminFull,
                Permission::AuditRead,
            ],
        }
    }

    /// Get the pricing for this tier
    pub fn price_per_month(&self) -> Option<u32> {
        match self {
            Tier::Free => Some(0),
            Tier::Developer => Some(99),
            Tier::Professional => Some(499),
            Tier::Enterprise => None, // Custom pricing
        }
    }

    /// Get feature description for this tier
    pub fn features(&self) -> Vec<&'static str> {
        match self {
            Tier::Free => vec![
                "Basic simulator access",
                "Limited execution quota",
                "Community support",
            ],
            Tier::Developer => vec![
                "All Free features",
                "Advanced simulator configuration",
                "Memory inspection",
                "Higher execution quota",
                "Email support",
            ],
            Tier::Professional => vec![
                "All Developer features",
                "Memory write access",
                "Unlimited execution quota",
                "Priority support",
                "Custom configurations",
            ],
            Tier::Enterprise => vec![
                "All Professional features",
                "Full administrative access",
                "Audit log access",
                "On-premise deployment option",
                "Dedicated support",
                "SLA guarantees",
                "Custom integrations",
            ],
        }
    }
}

/// Role manager for converting roles to permissions
pub struct RoleManager {
    role_to_permissions: HashMap<String, Vec<Permission>>,
}

impl RoleManager {
    /// Create a new role manager with default tier mappings
    pub fn new() -> Self {
        let mut role_to_permissions = HashMap::new();

        // Map tier names to their permissions
        role_to_permissions.insert("free".to_string(), Tier::Free.permissions());
        role_to_permissions.insert("developer".to_string(), Tier::Developer.permissions());
        role_to_permissions.insert("professional".to_string(), Tier::Professional.permissions());
        role_to_permissions.insert("enterprise".to_string(), Tier::Enterprise.permissions());

        // Additional special roles
        role_to_permissions.insert(
            "admin".to_string(),
            Tier::Enterprise.permissions(), // Admins get enterprise permissions
        );

        role_to_permissions.insert(
            "support".to_string(),
            vec![
                Permission::SimulatorRead,
                Permission::MemoryRead,
                Permission::AuditRead,
            ],
        );

        Self { role_to_permissions }
    }

    /// Get permissions for a role
    pub fn get_permissions(&self, role: &str) -> Vec<Permission> {
        self.role_to_permissions
            .get(&role.to_lowercase())
            .cloned()
            .unwrap_or_default()
    }

    /// Check if a role has a specific permission
    pub fn has_permission(&self, role: &str, permission: Permission) -> bool {
        self.get_permissions(role).contains(&permission)
    }

    /// Add a custom role with specific permissions
    pub fn add_role(&mut self, role: String, permissions: Vec<Permission>) {
        self.role_to_permissions.insert(role.to_lowercase(), permissions);
    }
}

impl Default for RoleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_hierarchy() {
        // Free tier
        let free_perms = Tier::Free.permissions();
        assert_eq!(free_perms.len(), 2);
        assert!(free_perms.contains(&Permission::SimulatorRead));
        assert!(free_perms.contains(&Permission::SimulatorExecute));

        // Developer includes Free + more
        let dev_perms = Tier::Developer.permissions();
        assert_eq!(dev_perms.len(), 4);
        assert!(dev_perms.contains(&Permission::SimulatorRead));
        assert!(dev_perms.contains(&Permission::SimulatorExecute));
        assert!(dev_perms.contains(&Permission::SimulatorConfigure));
        assert!(dev_perms.contains(&Permission::MemoryRead));

        // Professional includes Developer + more
        let pro_perms = Tier::Professional.permissions();
        assert_eq!(pro_perms.len(), 5);
        assert!(pro_perms.contains(&Permission::MemoryWrite));

        // Enterprise includes Professional + more
        let ent_perms = Tier::Enterprise.permissions();
        assert_eq!(ent_perms.len(), 7);
        assert!(ent_perms.contains(&Permission::AdminFull));
        assert!(ent_perms.contains(&Permission::AuditRead));
    }

    #[test]
    fn test_tier_pricing() {
        assert_eq!(Tier::Free.price_per_month(), Some(0));
        assert_eq!(Tier::Developer.price_per_month(), Some(99));
        assert_eq!(Tier::Professional.price_per_month(), Some(499));
        assert_eq!(Tier::Enterprise.price_per_month(), None);
    }

    #[test]
    fn test_role_manager() {
        let manager = RoleManager::new();

        // Free role
        let free_perms = manager.get_permissions("free");
        assert_eq!(free_perms.len(), 2);

        // Developer role
        let dev_perms = manager.get_permissions("developer");
        assert_eq!(dev_perms.len(), 4);

        // Case insensitive
        let dev_perms_upper = manager.get_permissions("DEVELOPER");
        assert_eq!(dev_perms_upper.len(), 4);

        // Unknown role returns empty
        let unknown = manager.get_permissions("unknown");
        assert_eq!(unknown.len(), 0);
    }

    #[test]
    fn test_role_manager_has_permission() {
        let manager = RoleManager::new();

        // Free user can read
        assert!(manager.has_permission("free", Permission::SimulatorRead));

        // Free user cannot configure
        assert!(!manager.has_permission("free", Permission::SimulatorConfigure));

        // Developer can configure
        assert!(manager.has_permission("developer", Permission::SimulatorConfigure));

        // Only enterprise has admin
        assert!(!manager.has_permission("professional", Permission::AdminFull));
        assert!(manager.has_permission("enterprise", Permission::AdminFull));
    }

    #[test]
    fn test_custom_role() {
        let mut manager = RoleManager::new();

        manager.add_role(
            "custom_auditor".to_string(),
            vec![Permission::AuditRead, Permission::SimulatorRead],
        );

        let perms = manager.get_permissions("custom_auditor");
        assert_eq!(perms.len(), 2);
        assert!(perms.contains(&Permission::AuditRead));
        assert!(perms.contains(&Permission::SimulatorRead));
    }
}
