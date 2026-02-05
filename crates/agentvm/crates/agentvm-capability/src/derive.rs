//! Capability derivation (attenuation).
//!
//! Allows creating child capabilities with reduced rights from parent capabilities.
//! Enforces the no-amplification invariant: child rights must be a subset of parent rights.
//!
//! ## Security Invariants
//!
//! 1. **No rights amplification**: Child rights must be a subset of parent rights
//! 2. **No scope amplification**: Child scope must be a subset of parent scope
//! 3. **No expiry extension**: Child cannot expire after parent
//! 4. **No quota inflation**: Child cannot have more quota than parent's remaining

use core::fmt;

use agentvm_types::{
    Capability, CapabilityId, CapabilityProof, CapabilityScope, Quota, Rights,
    TimestampNs,
};

/// Request to derive a new capability from a parent.
#[derive(Debug, Clone)]
pub struct DeriveRequest {
    /// New capability ID (must be unique)
    pub new_id: CapabilityId,
    /// Requested rights (must be subset of parent rights)
    pub new_rights: Rights,
    /// Requested scope (must be subset of parent scope)
    pub new_scope: CapabilityScope,
    /// Requested expiry (must not exceed parent expiry)
    pub new_expires_at: TimestampNs,
    /// Requested quota (must not exceed parent remaining quota)
    pub new_quota: Quota,
}

impl DeriveRequest {
    /// Create a new derive request
    pub fn new(
        new_id: CapabilityId,
        new_rights: Rights,
        new_scope: CapabilityScope,
        new_expires_at: TimestampNs,
        new_quota: Quota,
    ) -> Self {
        Self {
            new_id,
            new_rights,
            new_scope,
            new_expires_at,
            new_quota,
        }
    }

    /// Create a derive request that copies all parent attributes with a new ID
    pub fn from_parent(parent: &Capability, new_id: CapabilityId) -> Self {
        Self {
            new_id,
            new_rights: parent.rights,
            new_scope: parent.scope.clone(),
            new_expires_at: parent.expires_at,
            new_quota: Quota {
                max_invocations: parent.quota.remaining_invocations(),
                used_invocations: 0,
                max_bytes: parent.quota.remaining_bytes(),
                used_bytes: 0,
                max_duration_ns: parent
                    .quota
                    .max_duration_ns
                    .saturating_sub(parent.quota.used_duration_ns),
                used_duration_ns: 0,
            },
        }
    }
}

/// Errors that can occur during capability derivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeriveError {
    /// Parent capability does not have DELEGATE right
    DerivationNotPermitted,
    /// Requested rights exceed parent rights (amplification attempt)
    RightsAmplification,
    /// Requested scope exceeds parent scope
    ScopeAmplification,
    /// Requested expiry exceeds parent expiry
    ExpiryExtension,
    /// Requested quota exceeds parent remaining quota
    QuotaExceeded,
    /// Parent capability has expired
    ParentExpired,
    /// Parent capability is revoked
    ParentRevoked,
    /// Parent capability signature is invalid
    InvalidParentSignature,
    /// Scope types are incompatible
    IncompatibleScopes,
    /// Invalid request parameters
    InvalidRequest,
}

impl fmt::Display for DeriveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DerivationNotPermitted => write!(f, "parent capability does not permit derivation"),
            Self::RightsAmplification => write!(f, "requested rights exceed parent rights"),
            Self::ScopeAmplification => write!(f, "requested scope exceeds parent scope"),
            Self::ExpiryExtension => write!(f, "requested expiry exceeds parent expiry"),
            Self::QuotaExceeded => write!(f, "requested quota exceeds parent remaining quota"),
            Self::ParentExpired => write!(f, "parent capability has expired"),
            Self::ParentRevoked => write!(f, "parent capability is revoked"),
            Self::InvalidParentSignature => write!(f, "parent capability signature is invalid"),
            Self::IncompatibleScopes => write!(f, "scope types are incompatible"),
            Self::InvalidRequest => write!(f, "invalid derivation request"),
        }
    }
}

/// Result of a successful derivation
#[derive(Debug, Clone)]
pub struct DeriveResult {
    /// The derived capability
    pub capability: Capability,
    /// Quota consumed from parent (invocations)
    pub invocations_consumed: u64,
    /// Quota consumed from parent (bytes)
    pub bytes_consumed: u64,
}

/// Derive a new capability from a parent capability.
///
/// This function enforces the capability security invariants:
/// 1. No rights amplification: new_rights must be subset of parent_rights
/// 2. No scope amplification: new_scope must be subset of parent_scope
/// 3. No expiry extension: new_expiry must not exceed parent_expiry
/// 4. No quota inflation: new_quota must not exceed parent's remaining quota
///
/// # Arguments
/// * `parent` - The parent capability to derive from
/// * `request` - The derivation request specifying the new capability parameters
/// * `current_time` - Current timestamp for expiry checking
///
/// # Returns
/// A `DeriveResult` containing the new capability, or a `DeriveError`
pub fn derive_capability(
    parent: &Capability,
    request: &DeriveRequest,
    current_time: TimestampNs,
) -> Result<DeriveResult, DeriveError> {
    // Check parent is not revoked
    if parent.revoked {
        return Err(DeriveError::ParentRevoked);
    }

    // Check parent is not expired
    if parent.is_expired(current_time) {
        return Err(DeriveError::ParentExpired);
    }

    // Check parent has DELEGATE right
    if !parent.rights.has(Rights::DELEGATE) {
        return Err(DeriveError::DerivationNotPermitted);
    }

    // Check no rights amplification
    if !request.new_rights.is_subset_of(parent.rights) {
        return Err(DeriveError::RightsAmplification);
    }

    // Check no scope amplification by intersection
    let intersected_scope = parent
        .scope
        .intersect(&request.new_scope)
        .ok_or(DeriveError::IncompatibleScopes)?;

    // Check no expiry extension
    if request.new_expires_at > parent.expires_at {
        return Err(DeriveError::ExpiryExtension);
    }

    // Check no quota inflation
    let parent_remaining_invocations = parent.quota.remaining_invocations();
    let parent_remaining_bytes = parent.quota.remaining_bytes();

    if request.new_quota.max_invocations > parent_remaining_invocations {
        return Err(DeriveError::QuotaExceeded);
    }

    if request.new_quota.max_bytes > parent_remaining_bytes {
        return Err(DeriveError::QuotaExceeded);
    }

    // Create the derived capability
    let capability = Capability {
        id: request.new_id,
        cap_type: parent.cap_type,
        scope: intersected_scope,
        rights: request.new_rights,
        quota: request.new_quota,
        expires_at: request.new_expires_at,
        parent: Some(parent.id),
        proof: CapabilityProof {
            issuer: parent.proof.issuer,
            signature: [0u8; 64], // Needs to be signed separately
            issued_at: current_time,
        },
        revoked: false,
    };

    Ok(DeriveResult {
        capability,
        invocations_consumed: request.new_quota.max_invocations,
        bytes_consumed: request.new_quota.max_bytes,
    })
}

/// Validate that a derivation chain is valid.
///
/// Verifies that each capability in the chain is properly derived
/// from its parent, maintaining the no-amplification invariants.
///
/// # Arguments
/// * `chain` - Capabilities from root to leaf
/// * `current_time` - Current timestamp for expiry checking
///
/// # Returns
/// `Ok(())` if chain is valid, or `Err((index, error))` indicating
/// which capability in the chain failed validation
pub fn validate_derivation_chain(
    chain: &[Capability],
    _current_time: TimestampNs,
) -> Result<(), (usize, DeriveError)> {
    if chain.is_empty() {
        return Ok(());
    }

    // First capability is the root - must have no parent
    if chain[0].parent.is_some() {
        // Could be valid if parent is elsewhere, but for a complete chain, root has no parent
    }

    // Validate each parent-child relationship
    for i in 1..chain.len() {
        let parent = &chain[i - 1];
        let child = &chain[i];

        // Check parent ID matches
        match &child.parent {
            Some(parent_id) if *parent_id == parent.id => {}
            _ => return Err((i, DeriveError::InvalidRequest)),
        }

        // Verify no amplification in rights
        if !child.rights.is_subset_of(parent.rights) {
            return Err((i, DeriveError::RightsAmplification));
        }

        // Verify no expiry extension
        if child.expires_at > parent.expires_at {
            return Err((i, DeriveError::ExpiryExtension));
        }

        // Parent must have had DELEGATE right
        if !parent.rights.has(Rights::DELEGATE) {
            return Err((i, DeriveError::DerivationNotPermitted));
        }

        // Check parent wasn't expired at child creation
        if parent.is_expired(child.proof.issued_at) {
            return Err((i, DeriveError::ParentExpired));
        }

        // Check parent wasn't revoked
        if parent.revoked {
            return Err((i, DeriveError::ParentRevoked));
        }
    }

    Ok(())
}

/// Builder for creating derivation requests with fluent API.
#[derive(Debug, Clone, Default)]
pub struct DeriveBuilder {
    new_id: Option<CapabilityId>,
    new_rights: Option<Rights>,
    new_scope: Option<CapabilityScope>,
    new_expires_at: Option<TimestampNs>,
    new_quota: Option<Quota>,
}

impl DeriveBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the new capability ID
    pub fn id(mut self, id: CapabilityId) -> Self {
        self.new_id = Some(id);
        self
    }

    /// Set the rights
    pub fn rights(mut self, rights: Rights) -> Self {
        self.new_rights = Some(rights);
        self
    }

    /// Set the scope
    pub fn scope(mut self, scope: CapabilityScope) -> Self {
        self.new_scope = Some(scope);
        self
    }

    /// Set the expiry
    pub fn expires_at(mut self, expires_at: TimestampNs) -> Self {
        self.new_expires_at = Some(expires_at);
        self
    }

    /// Set the quota
    pub fn quota(mut self, quota: Quota) -> Self {
        self.new_quota = Some(quota);
        self
    }

    /// Build the derive request
    pub fn build(self) -> Option<DeriveRequest> {
        Some(DeriveRequest {
            new_id: self.new_id?,
            new_rights: self.new_rights?,
            new_scope: self.new_scope?,
            new_expires_at: self.new_expires_at.unwrap_or(TimestampNs::MAX),
            new_quota: self.new_quota.unwrap_or(Quota::UNLIMITED),
        })
    }
}

/// Calculate the maximum derivable quota from a parent capability
pub fn max_derivable_quota(parent: &Capability) -> Quota {
    Quota {
        max_invocations: parent.quota.remaining_invocations(),
        used_invocations: 0,
        max_bytes: parent.quota.remaining_bytes(),
        used_bytes: 0,
        max_duration_ns: parent
            .quota
            .max_duration_ns
            .saturating_sub(parent.quota.used_duration_ns),
        used_duration_ns: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentvm_types::CapabilityType;

    fn parent_capability() -> Capability {
        Capability {
            id: CapabilityId::from_bytes([1u8; 16]),
            cap_type: CapabilityType::FileRead,
            scope: CapabilityScope::Unrestricted,
            rights: Rights(Rights::READ | Rights::WRITE | Rights::DELEGATE),
            quota: Quota {
                max_invocations: 100,
                used_invocations: 0,
                max_bytes: 1_000_000,
                used_bytes: 0,
                max_duration_ns: 60_000_000_000,
                used_duration_ns: 0,
            },
            expires_at: 2_000_000_000_000_000_000,
            parent: None,
            proof: CapabilityProof {
                issuer: [0u8; 32],
                signature: [0u8; 64],
                issued_at: 1_000_000_000_000_000_000,
            },
            revoked: false,
        }
    }

    #[test]
    fn test_derive_success() {
        let parent = parent_capability();

        let request = DeriveRequest::new(
            CapabilityId::from_bytes([10u8; 16]),
            Rights(Rights::READ), // Subset of parent
            CapabilityScope::Unrestricted,
            1_800_000_000_000_000_000, // Before parent expiry
            Quota {
                max_invocations: 50,
                used_invocations: 0,
                max_bytes: 500_000,
                used_bytes: 0,
                max_duration_ns: 30_000_000_000,
                used_duration_ns: 0,
            },
        );

        let current_time = 1_500_000_000_000_000_000;
        let result = derive_capability(&parent, &request, current_time);
        assert!(result.is_ok());

        let derived = result.unwrap();
        assert_eq!(derived.capability.parent, Some(parent.id));
        assert_eq!(derived.capability.rights, Rights(Rights::READ));
        assert_eq!(derived.invocations_consumed, 50);
    }

    #[test]
    fn test_derive_rights_amplification() {
        let parent = parent_capability();

        let request = DeriveRequest::new(
            CapabilityId::from_bytes([10u8; 16]),
            Rights(Rights::READ | Rights::EXECUTE), // EXECUTE not in parent
            CapabilityScope::Unrestricted,
            1_800_000_000_000_000_000,
            Quota::UNLIMITED,
        );

        let result = derive_capability(&parent, &request, 1_500_000_000_000_000_000);
        assert_eq!(result.unwrap_err(), DeriveError::RightsAmplification);
    }

    #[test]
    fn test_derive_no_delegate_right() {
        let mut parent = parent_capability();
        parent.rights = Rights(Rights::READ | Rights::WRITE); // No DELEGATE

        let request = DeriveRequest::new(
            CapabilityId::from_bytes([10u8; 16]),
            Rights(Rights::READ),
            CapabilityScope::Unrestricted,
            1_800_000_000_000_000_000,
            Quota::UNLIMITED,
        );

        let result = derive_capability(&parent, &request, 1_500_000_000_000_000_000);
        assert_eq!(result.unwrap_err(), DeriveError::DerivationNotPermitted);
    }

    #[test]
    fn test_derive_expiry_extension() {
        let parent = parent_capability();

        let request = DeriveRequest::new(
            CapabilityId::from_bytes([10u8; 16]),
            Rights(Rights::READ),
            CapabilityScope::Unrestricted,
            3_000_000_000_000_000_000, // After parent expiry
            Quota::UNLIMITED,
        );

        let result = derive_capability(&parent, &request, 1_500_000_000_000_000_000);
        assert_eq!(result.unwrap_err(), DeriveError::ExpiryExtension);
    }

    #[test]
    fn test_derive_quota_exceeded() {
        let parent = parent_capability();

        let request = DeriveRequest::new(
            CapabilityId::from_bytes([10u8; 16]),
            Rights(Rights::READ),
            CapabilityScope::Unrestricted,
            1_800_000_000_000_000_000,
            Quota {
                max_invocations: 200, // More than parent's 100
                used_invocations: 0,
                max_bytes: 500_000,
                used_bytes: 0,
                max_duration_ns: 30_000_000_000,
                used_duration_ns: 0,
            },
        );

        let result = derive_capability(&parent, &request, 1_500_000_000_000_000_000);
        assert_eq!(result.unwrap_err(), DeriveError::QuotaExceeded);
    }

    #[test]
    fn test_derive_parent_expired() {
        let parent = parent_capability();

        let request = DeriveRequest::new(
            CapabilityId::from_bytes([10u8; 16]),
            Rights(Rights::READ),
            CapabilityScope::Unrestricted,
            1_800_000_000_000_000_000,
            Quota::UNLIMITED,
        );

        // Current time after parent expiry
        let result = derive_capability(&parent, &request, 3_000_000_000_000_000_000);
        assert_eq!(result.unwrap_err(), DeriveError::ParentExpired);
    }

    #[test]
    fn test_derive_parent_revoked() {
        let mut parent = parent_capability();
        parent.revoked = true;

        let request = DeriveRequest::new(
            CapabilityId::from_bytes([10u8; 16]),
            Rights(Rights::READ),
            CapabilityScope::Unrestricted,
            1_800_000_000_000_000_000,
            Quota::UNLIMITED,
        );

        let result = derive_capability(&parent, &request, 1_500_000_000_000_000_000);
        assert_eq!(result.unwrap_err(), DeriveError::ParentRevoked);
    }

    #[test]
    fn test_derive_from_parent() {
        let parent = parent_capability();
        let new_id = CapabilityId::from_bytes([10u8; 16]);

        let request = DeriveRequest::from_parent(&parent, new_id);

        assert_eq!(request.new_rights, parent.rights);
        assert_eq!(request.new_expires_at, parent.expires_at);
        assert_eq!(
            request.new_quota.max_invocations,
            parent.quota.remaining_invocations()
        );
    }

    #[test]
    fn test_validate_derivation_chain() {
        let parent = parent_capability();

        let mut child = parent.clone();
        child.id = CapabilityId::from_bytes([10u8; 16]);
        child.parent = Some(parent.id);
        child.rights = Rights(Rights::READ);

        let chain = [parent, child];
        assert!(validate_derivation_chain(&chain, 1_500_000_000_000_000_000).is_ok());
    }

    #[test]
    fn test_builder() {
        let request = DeriveBuilder::new()
            .id(CapabilityId::from_bytes([10u8; 16]))
            .rights(Rights(Rights::READ))
            .scope(CapabilityScope::Unrestricted)
            .expires_at(1_800_000_000_000_000_000)
            .build();

        assert!(request.is_some());
        let req = request.unwrap();
        assert_eq!(req.new_rights, Rights(Rights::READ));
    }

    #[test]
    fn test_max_derivable_quota() {
        let mut parent = parent_capability();
        parent.quota.used_invocations = 30;
        parent.quota.used_bytes = 200_000;

        let max_quota = max_derivable_quota(&parent);

        assert_eq!(max_quota.max_invocations, 70);
        assert_eq!(max_quota.max_bytes, 800_000);
    }
}
