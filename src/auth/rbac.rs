//! Role-Based Access Control (RBAC) implementation for Cognitum chip v1
//!
//! This module provides comprehensive permission management with:
//! - Permission-based access control
//! - Resource-level authorization
//! - Deny-by-default policy
//! - Tier-based permission inheritance

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

#[cfg(test)]
use mockall::automock;

/// Errors that can occur during RBAC operations
#[derive(Debug, Clone, PartialEq)]
pub enum RbacError {
    /// Permission denied for the requested operation
    PermissionDenied {
        permission: Permission,
        user_id: String,
    },
    /// Resource not found
    ResourceNotFound(String),
    /// User not found
    UserNotFound(String),
    /// Store operation failed
    StoreError(String),
    /// Invalid permission request
    InvalidPermission(String),
}

impl fmt::Display for RbacError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RbacError::PermissionDenied { permission, user_id } => {
                write!(f, "Permission denied: user {} cannot {}", user_id, permission)
            }
            RbacError::ResourceNotFound(id) => write!(f, "Resource not found: {}", id),
            RbacError::UserNotFound(id) => write!(f, "User not found: {}", id),
            RbacError::StoreError(msg) => write!(f, "Store error: {}", msg),
            RbacError::InvalidPermission(msg) => write!(f, "Invalid permission: {}", msg),
        }
    }
}

impl std::error::Error for RbacError {}

/// Permissions available in the Cognitum system
///
/// Permissions follow a hierarchical model where higher-tier permissions
/// include lower-tier capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// Read simulator state (Free tier+)
    SimulatorRead,
    /// Execute simulations (Free tier+ with limits)
    SimulatorExecute,
    /// Configure simulator parameters (Developer tier+)
    SimulatorConfigure,
    /// Read memory state (Developer tier+)
    MemoryRead,
    /// Write to memory (Professional tier+)
    MemoryWrite,
    /// Full administrative access (Enterprise tier only)
    AdminFull,
    /// Read audit logs (Enterprise tier only)
    AuditRead,
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Permission::SimulatorRead => write!(f, "simulator:read"),
            Permission::SimulatorExecute => write!(f, "simulator:execute"),
            Permission::SimulatorConfigure => write!(f, "simulator:configure"),
            Permission::MemoryRead => write!(f, "memory:read"),
            Permission::MemoryWrite => write!(f, "memory:write"),
            Permission::AdminFull => write!(f, "admin:full"),
            Permission::AuditRead => write!(f, "audit:read"),
        }
    }
}

/// User identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserId(String);

impl UserId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UserId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Resource identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResourceId(String);

impl ResourceId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// User with role information
#[derive(Debug, Clone)]
pub struct User {
    pub id: UserId,
    pub roles: Vec<String>,
}

/// Store error type
#[derive(Debug, Clone, PartialEq)]
pub enum StoreError {
    NotFound,
    DatabaseError(String),
    ConnectionError,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::NotFound => write!(f, "Not found"),
            StoreError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            StoreError::ConnectionError => write!(f, "Connection error"),
        }
    }
}

impl std::error::Error for StoreError {}

/// Trait for permission storage backend
///
/// This trait is mockable for testing purposes using mockall
#[cfg_attr(test, automock)]
pub trait PermissionStore: Send + Sync {
    /// Get all permissions for a role
    fn get_role_permissions(&self, role: &str) -> Result<Vec<Permission>, StoreError>;

    /// Check if a user has a specific permission
    fn check_permission(&self, user_id: &UserId, permission: Permission) -> Result<bool, StoreError>;

    /// Check if a user owns a resource
    fn check_resource_ownership(&self, user_id: &UserId, resource_id: &ResourceId) -> Result<bool, StoreError>;
}

/// RBAC Service for permission checking
///
/// Implements deny-by-default security model where:
/// - All permissions are denied unless explicitly granted
/// - Resource access requires ownership verification
/// - Errors default to denial
pub struct RbacService {
    store: Arc<dyn PermissionStore>,
}

impl RbacService {
    /// Create a new RBAC service with the given permission store
    pub fn new(store: Arc<dyn PermissionStore>) -> Self {
        Self { store }
    }

    /// Check if a user has a specific permission
    ///
    /// Implements deny-by-default: returns false on any error
    ///
    /// # Arguments
    /// * `user` - The user to check
    /// * `permission` - The permission to verify
    ///
    /// # Returns
    /// `Ok(true)` if permission is granted, `Ok(false)` if denied
    pub async fn can(&self, user: &User, permission: Permission) -> Result<bool, RbacError> {
        // Collect all permissions from all user roles
        let mut all_permissions = HashSet::new();

        for role in &user.roles {
            match self.store.get_role_permissions(role) {
                Ok(perms) => {
                    all_permissions.extend(perms);
                }
                Err(StoreError::NotFound) => {
                    // Unknown role - deny by default
                    continue;
                }
                Err(_e) => {
                    // Store error - deny by default
                    return Ok(false);
                }
            }
        }

        // Check if permission is in the set
        Ok(all_permissions.contains(&permission))
    }

    /// Check if a user can access a specific resource
    ///
    /// Implements resource-level access control with ownership verification
    ///
    /// # Arguments
    /// * `user_id` - The user requesting access
    /// * `resource_id` - The resource to access
    ///
    /// # Returns
    /// `Ok(true)` if access is granted, `Ok(false)` if denied
    pub async fn can_access_resource(
        &self,
        user_id: &UserId,
        resource_id: &ResourceId,
    ) -> Result<bool, RbacError> {
        match self.store.check_resource_ownership(user_id, resource_id) {
            Ok(owns) => Ok(owns),
            Err(StoreError::NotFound) => {
                // Resource or user not found - deny by default
                Ok(false)
            }
            Err(_e) => {
                // Store error - deny by default
                Ok(false)
            }
        }
    }

    /// Require a specific permission, returning an error if denied
    ///
    /// # Arguments
    /// * `user` - The user to check
    /// * `permission` - The required permission
    ///
    /// # Errors
    /// Returns `RbacError::PermissionDenied` if the user lacks the permission
    pub async fn require(&self, user: &User, permission: Permission) -> Result<(), RbacError> {
        let has_permission = self.can(user, permission).await?;

        if has_permission {
            Ok(())
        } else {
            Err(RbacError::PermissionDenied {
                permission,
                user_id: user.id.to_string(),
            })
        }
    }

    /// Require resource access, returning an error if denied
    ///
    /// # Arguments
    /// * `user_id` - The user requesting access
    /// * `resource_id` - The resource to access
    ///
    /// # Errors
    /// Returns `RbacError::PermissionDenied` if access is not allowed
    pub async fn require_resource_access(
        &self,
        user_id: &UserId,
        resource_id: &ResourceId,
    ) -> Result<(), RbacError> {
        let can_access = self.can_access_resource(user_id, resource_id).await?;

        if can_access {
            Ok(())
        } else {
            Err(RbacError::PermissionDenied {
                permission: Permission::SimulatorRead, // Generic permission for error
                user_id: user_id.to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_deny_by_default_on_store_error() {
        let mut mock_store = MockPermissionStore::new();

        mock_store
            .expect_get_role_permissions()
            .returning(|_| Err(StoreError::DatabaseError("connection failed".to_string())));

        let rbac = RbacService::new(Arc::new(mock_store));

        let user = User {
            id: UserId::new("user_123"),
            roles: vec!["admin".to_string()],
        };

        // Should deny on error
        let result = rbac.can(&user, Permission::AdminFull).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_deny_by_default_on_unknown_role() {
        let mut mock_store = MockPermissionStore::new();

        mock_store
            .expect_get_role_permissions()
            .returning(|_| Err(StoreError::NotFound));

        let rbac = RbacService::new(Arc::new(mock_store));

        let user = User {
            id: UserId::new("user_123"),
            roles: vec!["unknown_role".to_string()],
        };

        // Should deny unknown role
        let result = rbac.can(&user, Permission::AdminFull).await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_resource_access_deny_on_not_found() {
        let mut mock_store = MockPermissionStore::new();

        mock_store
            .expect_check_resource_ownership()
            .returning(|_, _| Err(StoreError::NotFound));

        let rbac = RbacService::new(Arc::new(mock_store));

        // Should deny when resource not found
        let result = rbac
            .can_access_resource(&UserId::new("user_123"), &ResourceId::new("resource_456"))
            .await
            .unwrap();

        assert!(!result);
    }
}
