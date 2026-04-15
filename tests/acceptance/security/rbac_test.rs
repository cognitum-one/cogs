//! Acceptance tests for RBAC (Role-Based Access Control)
//!
//! These tests verify the permission enforcement following the security TDD plan
//! (Phase 2.2) from 09_SECURITY_TDD.md

use cognitum::auth::{Permission, PermissionStore, RbacService, ResourceId, StoreError, User, UserId};
use std::sync::Arc;

#[cfg(test)]
use mockall::automock;
use mockall::predicate::*;

#[cfg_attr(test, automock)]
pub trait PermissionStoreMock: Send + Sync {
    fn get_role_permissions(&self, role: &str) -> Result<Vec<Permission>, StoreError>;
    fn check_permission(&self, user_id: &UserId, permission: Permission) -> Result<bool, StoreError>;
    fn check_resource_ownership(&self, user_id: &UserId, resource_id: &ResourceId) -> Result<bool, StoreError>;
}

// Implement the actual PermissionStore trait for our mock
struct MockStoreAdapter {
    mock: Arc<dyn PermissionStoreMock>,
}

impl PermissionStore for MockStoreAdapter {
    fn get_role_permissions(&self, role: &str) -> Result<Vec<Permission>, StoreError> {
        self.mock.get_role_permissions(role)
    }

    fn check_permission(&self, user_id: &UserId, permission: Permission) -> Result<bool, StoreError> {
        self.mock.check_permission(user_id, permission)
    }

    fn check_resource_ownership(&self, user_id: &UserId, resource_id: &ResourceId) -> Result<bool, StoreError> {
        self.mock.check_resource_ownership(user_id, resource_id)
    }
}

#[cfg(test)]
mod rbac_tests {
    use super::*;

    #[tokio::test]
    async fn free_tier_cannot_access_enterprise_features() {
        let mut mock_store = MockPermissionStoreMock::new();

        mock_store
            .expect_get_role_permissions()
            .with(eq("free"))
            .returning(|_| {
                Ok(vec![
                    Permission::SimulatorRead,
                    Permission::SimulatorExecute,
                ])
            });

        let adapter = MockStoreAdapter {
            mock: Arc::new(mock_store),
        };
        let rbac = RbacService::new(Arc::new(adapter));

        let user = User {
            id: UserId::new("user_123"),
            roles: vec!["free".to_string()],
        };

        // Free tier CAN execute basic simulation
        assert!(rbac.can(&user, Permission::SimulatorExecute).await.unwrap());

        // Free tier CANNOT configure advanced settings
        assert!(!rbac.can(&user, Permission::SimulatorConfigure).await.unwrap());

        // Free tier CANNOT access audit logs
        assert!(!rbac.can(&user, Permission::AuditRead).await.unwrap());
    }

    #[tokio::test]
    async fn developer_tier_can_configure_but_not_write_memory() {
        let mut mock_store = MockPermissionStoreMock::new();

        mock_store
            .expect_get_role_permissions()
            .with(eq("developer"))
            .returning(|_| {
                Ok(vec![
                    Permission::SimulatorRead,
                    Permission::SimulatorExecute,
                    Permission::SimulatorConfigure,
                    Permission::MemoryRead,
                ])
            });

        let adapter = MockStoreAdapter {
            mock: Arc::new(mock_store),
        };
        let rbac = RbacService::new(Arc::new(adapter));

        let user = User {
            id: UserId::new("dev_user_456"),
            roles: vec!["developer".to_string()],
        };

        // Developer CAN configure
        assert!(rbac.can(&user, Permission::SimulatorConfigure).await.unwrap());

        // Developer CAN read memory
        assert!(rbac.can(&user, Permission::MemoryRead).await.unwrap());

        // Developer CANNOT write memory
        assert!(!rbac.can(&user, Permission::MemoryWrite).await.unwrap());

        // Developer CANNOT access admin features
        assert!(!rbac.can(&user, Permission::AdminFull).await.unwrap());
    }

    #[tokio::test]
    async fn professional_tier_can_write_memory() {
        let mut mock_store = MockPermissionStoreMock::new();

        mock_store
            .expect_get_role_permissions()
            .with(eq("professional"))
            .returning(|_| {
                Ok(vec![
                    Permission::SimulatorRead,
                    Permission::SimulatorExecute,
                    Permission::SimulatorConfigure,
                    Permission::MemoryRead,
                    Permission::MemoryWrite,
                ])
            });

        let adapter = MockStoreAdapter {
            mock: Arc::new(mock_store),
        };
        let rbac = RbacService::new(Arc::new(adapter));

        let user = User {
            id: UserId::new("pro_user_789"),
            roles: vec!["professional".to_string()],
        };

        // Professional CAN write memory
        assert!(rbac.can(&user, Permission::MemoryWrite).await.unwrap());

        // Professional CAN read memory
        assert!(rbac.can(&user, Permission::MemoryRead).await.unwrap());

        // Professional CANNOT access admin features
        assert!(!rbac.can(&user, Permission::AdminFull).await.unwrap());

        // Professional CANNOT access audit logs
        assert!(!rbac.can(&user, Permission::AuditRead).await.unwrap());
    }

    #[tokio::test]
    async fn enterprise_tier_has_full_access() {
        let mut mock_store = MockPermissionStoreMock::new();

        mock_store
            .expect_get_role_permissions()
            .with(eq("enterprise"))
            .returning(|_| {
                Ok(vec![
                    Permission::SimulatorRead,
                    Permission::SimulatorExecute,
                    Permission::SimulatorConfigure,
                    Permission::MemoryRead,
                    Permission::MemoryWrite,
                    Permission::AdminFull,
                    Permission::AuditRead,
                ])
            });

        let adapter = MockStoreAdapter {
            mock: Arc::new(mock_store),
        };
        let rbac = RbacService::new(Arc::new(adapter));

        let user = User {
            id: UserId::new("ent_user_999"),
            roles: vec!["enterprise".to_string()],
        };

        // Enterprise has ALL permissions
        assert!(rbac.can(&user, Permission::SimulatorRead).await.unwrap());
        assert!(rbac.can(&user, Permission::SimulatorExecute).await.unwrap());
        assert!(rbac.can(&user, Permission::SimulatorConfigure).await.unwrap());
        assert!(rbac.can(&user, Permission::MemoryRead).await.unwrap());
        assert!(rbac.can(&user, Permission::MemoryWrite).await.unwrap());
        assert!(rbac.can(&user, Permission::AdminFull).await.unwrap());
        assert!(rbac.can(&user, Permission::AuditRead).await.unwrap());
    }

    #[tokio::test]
    async fn permission_check_uses_deny_by_default() {
        let mut mock_store = MockPermissionStoreMock::new();

        // Unknown permission returns error
        mock_store
            .expect_get_role_permissions()
            .returning(|_| Err(StoreError::NotFound));

        let adapter = MockStoreAdapter {
            mock: Arc::new(mock_store),
        };
        let rbac = RbacService::new(Arc::new(adapter));

        let user = User {
            id: UserId::new("user_123"),
            roles: vec!["unknown_role".to_string()],
        };

        // Unknown permissions are denied
        let result = rbac.can(&user, Permission::AdminFull).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn resource_level_permissions_are_enforced() {
        let mut mock_store = MockPermissionStoreMock::new();

        mock_store
            .expect_check_resource_ownership()
            .withf(|user_id, resource_id| {
                user_id.as_str() == "user_123" && resource_id.as_str() == "sim_abc"
            })
            .returning(|_, _| Ok(true));

        mock_store
            .expect_check_resource_ownership()
            .withf(|user_id, resource_id| {
                user_id.as_str() == "user_456" && resource_id.as_str() == "sim_abc"
            })
            .returning(|_, _| Ok(false));

        let adapter = MockStoreAdapter {
            mock: Arc::new(mock_store),
        };
        let rbac = RbacService::new(Arc::new(adapter));

        // User 123 owns resource, can access
        assert!(
            rbac.can_access_resource(&UserId::new("user_123"), &ResourceId::new("sim_abc"))
                .await
                .unwrap()
        );

        // User 456 does not own resource, denied
        assert!(
            !rbac
                .can_access_resource(&UserId::new("user_456"), &ResourceId::new("sim_abc"))
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn multiple_roles_combine_permissions() {
        let mut mock_store = MockPermissionStoreMock::new();

        // Setup expectations for both roles
        mock_store
            .expect_get_role_permissions()
            .with(eq("developer"))
            .returning(|_| {
                Ok(vec![
                    Permission::SimulatorConfigure,
                    Permission::MemoryRead,
                ])
            });

        mock_store
            .expect_get_role_permissions()
            .with(eq("support"))
            .returning(|_| Ok(vec![Permission::AuditRead]));

        let adapter = MockStoreAdapter {
            mock: Arc::new(mock_store),
        };
        let rbac = RbacService::new(Arc::new(adapter));

        let user = User {
            id: UserId::new("multi_role_user"),
            roles: vec!["developer".to_string(), "support".to_string()],
        };

        // Should have permissions from both roles
        assert!(rbac.can(&user, Permission::SimulatorConfigure).await.unwrap());
        assert!(rbac.can(&user, Permission::MemoryRead).await.unwrap());
        assert!(rbac.can(&user, Permission::AuditRead).await.unwrap());

        // Should NOT have permissions not in either role
        assert!(!rbac.can(&user, Permission::AdminFull).await.unwrap());
    }

    #[tokio::test]
    async fn deny_on_resource_not_found() {
        let mut mock_store = MockPermissionStoreMock::new();

        mock_store
            .expect_check_resource_ownership()
            .returning(|_, _| Err(StoreError::NotFound));

        let adapter = MockStoreAdapter {
            mock: Arc::new(mock_store),
        };
        let rbac = RbacService::new(Arc::new(adapter));

        // Should deny when resource not found (deny-by-default)
        let result = rbac
            .can_access_resource(&UserId::new("user_123"), &ResourceId::new("nonexistent"))
            .await
            .unwrap();

        assert!(!result);
    }
}
