//! Role-based access control and capability tokens for FXNN governance.
//!
//! This module provides fine-grained access control for memory regions and
//! resources, implementing authority boundaries between agents.
//!
//! # Overview
//!
//! The permission system follows a role-based access control (RBAC) model:
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │                    PERMISSION HIERARCHY                         │
//! ├────────────────────────────────────────────────────────────────┤
//! │                                                                 │
//! │   Admin ──────┬─────────────────────────────────────────────── │
//! │               │                                                 │
//! │   Execute ────┼──────────────────────────────────────────────  │
//! │               │                                                 │
//! │   Write ──────┼─────────────────────────────────────────────   │
//! │               │                                                 │
//! │   Read ───────┴────────────────────────────────────────────    │
//! │                                                                 │
//! │   Higher permissions imply lower ones                           │
//! │                                                                 │
//! └────────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust
//! use fxnn::governance::{Permission, Role, MemoryRegion, CapabilityToken};
//!
//! // Create a role with specific permissions
//! let mut agent_role = Role::new("basic_agent");
//! agent_role.grant(Permission::Read);
//! agent_role.grant(Permission::Write);
//!
//! // Check permissions
//! assert!(agent_role.has_permission(&Permission::Read));
//! assert!(!agent_role.has_permission(&Permission::Admin));
//! ```

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use super::AgentId;

/// Permission levels for resources
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Permission {
    /// Read-only access
    Read = 0,
    /// Write access (implies Read)
    Write = 1,
    /// Execute access (implies Write and Read)
    Execute = 2,
    /// Administrative access (implies all others)
    Admin = 3,
}

impl Permission {
    /// Check if this permission level includes another
    pub fn includes(&self, other: &Permission) -> bool {
        *self >= *other
    }

    /// Get all permissions implied by this permission level
    pub fn implied_permissions(&self) -> Vec<Permission> {
        match self {
            Permission::Read => vec![Permission::Read],
            Permission::Write => vec![Permission::Read, Permission::Write],
            Permission::Execute => vec![Permission::Read, Permission::Write, Permission::Execute],
            Permission::Admin => vec![
                Permission::Read,
                Permission::Write,
                Permission::Execute,
                Permission::Admin,
            ],
        }
    }
}

/// A role that can be assigned to agents
#[derive(Debug, Clone)]
pub struct Role {
    /// Human-readable name for the role
    pub name: String,
    /// Role ID for efficient lookups
    pub id: u32,
    /// Set of permissions granted to this role
    permissions: HashSet<Permission>,
    /// Parent role (for inheritance)
    parent: Option<Box<Role>>,
}

impl Role {
    /// Create a new role with no permissions
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            id: 0,
            permissions: HashSet::new(),
            parent: None,
        }
    }

    /// Create a new role with a specific ID
    pub fn with_id(mut self, id: u32) -> Self {
        self.id = id;
        self
    }

    /// Set a parent role for inheritance
    pub fn with_parent(mut self, parent: Role) -> Self {
        self.parent = Some(Box::new(parent));
        self
    }

    /// Grant a permission to this role
    pub fn grant(&mut self, permission: Permission) {
        // When granting a permission, also grant all implied permissions
        for implied in permission.implied_permissions() {
            self.permissions.insert(implied);
        }
    }

    /// Revoke a permission from this role
    pub fn revoke(&mut self, permission: &Permission) {
        self.permissions.remove(permission);
        // Also revoke any permissions that imply this one
        match permission {
            Permission::Read => {
                self.permissions.remove(&Permission::Write);
                self.permissions.remove(&Permission::Execute);
                self.permissions.remove(&Permission::Admin);
            }
            Permission::Write => {
                self.permissions.remove(&Permission::Execute);
                self.permissions.remove(&Permission::Admin);
            }
            Permission::Execute => {
                self.permissions.remove(&Permission::Admin);
            }
            Permission::Admin => {}
        }
    }

    /// Check if this role has a specific permission
    pub fn has_permission(&self, permission: &Permission) -> bool {
        // Check own permissions
        if self.permissions.contains(permission) {
            return true;
        }

        // Check inherited permissions
        if let Some(ref parent) = self.parent {
            return parent.has_permission(permission);
        }

        false
    }

    /// Get all permissions for this role (including inherited)
    pub fn all_permissions(&self) -> HashSet<Permission> {
        let mut perms = self.permissions.clone();

        if let Some(ref parent) = self.parent {
            perms.extend(parent.all_permissions());
        }

        perms
    }

    /// Create a basic agent role
    pub fn basic_agent() -> Self {
        let mut role = Self::new("basic_agent").with_id(0);
        role.grant(Permission::Read);
        role
    }

    /// Create a privileged agent role
    pub fn privileged_agent() -> Self {
        let mut role = Self::new("privileged_agent")
            .with_id(1)
            .with_parent(Self::basic_agent());
        role.grant(Permission::Write);
        role
    }

    /// Create an admin role
    pub fn admin() -> Self {
        let mut role = Self::new("admin")
            .with_id(2)
            .with_parent(Self::privileged_agent());
        role.grant(Permission::Admin);
        role
    }
}

/// A memory region that can have access control
#[derive(Debug, Clone)]
pub struct MemoryRegion {
    /// Unique identifier for this region
    pub id: u64,
    /// Human-readable name
    pub name: String,
    /// Owner agent ID (has full access)
    pub owner_id: Option<AgentId>,
    /// Whether this region is shared (multiple readers allowed)
    pub is_shared: bool,
    /// Whether this region is read-only (no writes allowed)
    pub is_read_only: bool,
    /// Size of the region in bytes
    pub size_bytes: usize,
}

impl MemoryRegion {
    /// Create a new memory region
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            owner_id: None,
            is_shared: false,
            is_read_only: false,
            size_bytes: 0,
        }
    }

    /// Set the owner
    pub fn with_owner(mut self, owner_id: AgentId) -> Self {
        self.owner_id = Some(owner_id);
        self
    }

    /// Mark as shared
    pub fn shared(mut self) -> Self {
        self.is_shared = true;
        self
    }

    /// Mark as read-only
    pub fn read_only(mut self) -> Self {
        self.is_read_only = true;
        self
    }

    /// Set the size
    pub fn with_size(mut self, size_bytes: usize) -> Self {
        self.size_bytes = size_bytes;
        self
    }

    /// Check if an agent owns this region
    pub fn is_owner(&self, agent_id: AgentId) -> bool {
        self.owner_id == Some(agent_id)
    }
}

/// A signed capability token for authorized action requests
#[derive(Debug, Clone)]
pub struct CapabilityToken {
    /// Unique token identifier
    pub id: u64,
    /// Agent this token was issued to
    pub agent_id: AgentId,
    /// Permission level granted
    pub permission: Permission,
    /// Target resource (memory region, action type, etc.)
    pub target: CapabilityTarget,
    /// When the token was issued
    pub issued_at: Instant,
    /// Token expiration duration
    pub expires_in: Duration,
    /// Signature for verification (simplified as hash)
    signature: u64,
}

/// Target of a capability token
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CapabilityTarget {
    /// Access to a specific memory region
    MemoryRegion(u64),
    /// Permission to perform a specific action type
    ActionType(String),
    /// Access to all resources of a type
    ResourceType(String),
    /// Global access (admin only)
    Global,
}

impl CapabilityToken {
    /// Create a new capability token
    pub fn new(
        agent_id: AgentId,
        permission: Permission,
        target: CapabilityTarget,
        expires_in: Duration,
    ) -> Self {
        let id = Self::generate_id();
        let issued_at = Instant::now();

        // Generate a simple signature (in production, use cryptographic signing)
        let signature = Self::compute_signature(id, agent_id, &permission, &target);

        Self {
            id,
            agent_id,
            permission,
            target,
            issued_at,
            expires_in,
            signature,
        }
    }

    /// Generate a unique token ID
    fn generate_id() -> u64 {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        COUNTER.fetch_add(1, Ordering::Relaxed)
    }

    /// Compute a simple signature for the token
    fn compute_signature(
        id: u64,
        agent_id: AgentId,
        permission: &Permission,
        target: &CapabilityTarget,
    ) -> u64 {
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;

        let mut hasher = DefaultHasher::new();
        id.hash(&mut hasher);
        agent_id.hash(&mut hasher);
        (*permission as u8).hash(&mut hasher);
        target.hash(&mut hasher);
        hasher.finish()
    }

    /// Check if the token is expired
    pub fn is_expired(&self) -> bool {
        self.issued_at.elapsed() > self.expires_in
    }

    /// Verify the token's signature
    pub fn verify(&self) -> bool {
        let expected = Self::compute_signature(
            self.id,
            self.agent_id,
            &self.permission,
            &self.target,
        );
        self.signature == expected && !self.is_expired()
    }

    /// Check if the token grants access to a target
    pub fn grants_access_to(&self, target: &CapabilityTarget, required: &Permission) -> bool {
        if !self.verify() {
            return false;
        }

        // Check permission level
        if !self.permission.includes(required) {
            return false;
        }

        // Check target
        match (&self.target, target) {
            (CapabilityTarget::Global, _) => true,
            (CapabilityTarget::MemoryRegion(a), CapabilityTarget::MemoryRegion(b)) => a == b,
            (CapabilityTarget::ActionType(a), CapabilityTarget::ActionType(b)) => a == b,
            (CapabilityTarget::ResourceType(a), CapabilityTarget::ResourceType(b)) => a == b,
            _ => false,
        }
    }
}

/// Memory permission manager
#[derive(Debug, Clone)]
pub struct MemoryPermissions {
    /// Map from (agent_id, region_id) to permission level
    agent_permissions: HashMap<(AgentId, u64), Permission>,
    /// Map from role_id to default permission for regions
    role_defaults: HashMap<u32, Permission>,
    /// Set of regions and their metadata
    regions: HashMap<u64, MemoryRegion>,
}

impl MemoryPermissions {
    /// Create a new permission manager
    pub fn new() -> Self {
        Self {
            agent_permissions: HashMap::new(),
            role_defaults: HashMap::new(),
            regions: HashMap::new(),
        }
    }

    /// Register a memory region
    pub fn register_region(&mut self, region: MemoryRegion) {
        self.regions.insert(region.id, region);
    }

    /// Grant permission to an agent for a region
    pub fn grant_agent(&mut self, agent_id: AgentId, region_id: u64, permission: Permission) {
        self.agent_permissions.insert((agent_id, region_id), permission);
    }

    /// Revoke all permissions for an agent on a region
    pub fn revoke_agent(&mut self, agent_id: AgentId, region_id: u64) {
        self.agent_permissions.remove(&(agent_id, region_id));
    }

    /// Set default permission for a role
    pub fn set_role_default(&mut self, role_id: u32, permission: Permission) {
        self.role_defaults.insert(role_id, permission);
    }

    /// Check if an agent can read a region
    pub fn can_read(&self, agent_id: AgentId, region_id: u64) -> bool {
        self.check_permission(agent_id, region_id, &Permission::Read)
    }

    /// Check if an agent can write to a region
    pub fn can_write(&self, agent_id: AgentId, region_id: u64) -> bool {
        // Check if region is read-only
        if let Some(region) = self.regions.get(&region_id) {
            if region.is_read_only {
                return false;
            }
        }

        self.check_permission(agent_id, region_id, &Permission::Write)
    }

    /// Check if an agent has a specific permission
    fn check_permission(&self, agent_id: AgentId, region_id: u64, required: &Permission) -> bool {
        // Check if agent is owner
        if let Some(region) = self.regions.get(&region_id) {
            if region.is_owner(agent_id) {
                return true;
            }
        }

        // Check explicit agent permission
        if let Some(perm) = self.agent_permissions.get(&(agent_id, region_id)) {
            return perm.includes(required);
        }

        // Check if shared region allows read
        if let Some(region) = self.regions.get(&region_id) {
            if region.is_shared && *required == Permission::Read {
                return true;
            }
        }

        false
    }

    /// Get all regions an agent can access
    pub fn accessible_regions(&self, agent_id: AgentId) -> Vec<&MemoryRegion> {
        self.regions
            .values()
            .filter(|r| self.can_read(agent_id, r.id))
            .collect()
    }
}

impl Default for MemoryPermissions {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_hierarchy() {
        assert!(Permission::Admin.includes(&Permission::Read));
        assert!(Permission::Admin.includes(&Permission::Write));
        assert!(Permission::Write.includes(&Permission::Read));
        assert!(!Permission::Read.includes(&Permission::Write));
    }

    #[test]
    fn test_role_permissions() {
        let mut role = Role::new("test");
        role.grant(Permission::Write);

        assert!(role.has_permission(&Permission::Read));
        assert!(role.has_permission(&Permission::Write));
        assert!(!role.has_permission(&Permission::Admin));
    }

    #[test]
    fn test_role_inheritance() {
        let parent = Role::basic_agent();
        let role = Role::new("child").with_parent(parent);

        // Should inherit Read permission from parent
        assert!(role.has_permission(&Permission::Read));
    }

    #[test]
    fn test_capability_token() {
        let token = CapabilityToken::new(
            1,
            Permission::Write,
            CapabilityTarget::MemoryRegion(100),
            Duration::from_secs(3600),
        );

        assert!(token.verify());
        assert!(!token.is_expired());

        // Should grant Read access (implied by Write)
        assert!(token.grants_access_to(
            &CapabilityTarget::MemoryRegion(100),
            &Permission::Read
        ));

        // Should not grant access to different region
        assert!(!token.grants_access_to(
            &CapabilityTarget::MemoryRegion(200),
            &Permission::Read
        ));
    }

    #[test]
    fn test_memory_permissions() {
        let mut perms = MemoryPermissions::new();

        let region = MemoryRegion::new(1, "test").with_owner(100);
        perms.register_region(region);

        // Owner should have access
        assert!(perms.can_read(100, 1));
        assert!(perms.can_write(100, 1));

        // Non-owner should not have access
        assert!(!perms.can_read(200, 1));

        // Grant explicit access
        perms.grant_agent(200, 1, Permission::Read);
        assert!(perms.can_read(200, 1));
        assert!(!perms.can_write(200, 1));
    }

    #[test]
    fn test_shared_region() {
        let mut perms = MemoryPermissions::new();

        let region = MemoryRegion::new(1, "shared").shared();
        perms.register_region(region);

        // Any agent should be able to read shared region
        assert!(perms.can_read(1, 1));
        assert!(perms.can_read(999, 1));

        // But not write
        assert!(!perms.can_write(1, 1));
    }

    #[test]
    fn test_read_only_region() {
        let mut perms = MemoryPermissions::new();

        let region = MemoryRegion::new(1, "readonly").with_owner(100).read_only();
        perms.register_region(region);

        // Owner can read but not write
        assert!(perms.can_read(100, 1));
        assert!(!perms.can_write(100, 1));
    }
}
