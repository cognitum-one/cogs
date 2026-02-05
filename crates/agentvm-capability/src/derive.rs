//! Capability derivation (attenuation)
//!
//! Capabilities can only be derived (attenuated), never amplified.
//! A derived capability must have equal or lesser rights than its parent.

use agentvm_types::{
    Capability, CapabilityId, CapabilityProof, CapabilityScope, Quota, Rights,
};

/// Request to derive a child capability
#[derive(Debug, Clone)]
pub struct DeriveRequest {
    /// Desired scope (must be subset of parent)
    pub scope: CapabilityScope,
    /// Desired rights (must be subset of parent)
    pub rights: Rights,
    /// Desired quota (must be <= remaining parent quota)
    pub quota: Quota,
    /// Desired expiry (must be <= parent expiry)
    pub expires_at: Option<u64>,
}

impl DeriveRequest {
    /// Create a new derive request
    pub fn new() -> Self {
        Self {
            scope: CapabilityScope::Global,
            rights: Rights::none(),
            quota: Quota::default(),
            expires_at: None,
        }
    }

    /// Set the scope
    pub fn with_scope(mut self, scope: CapabilityScope) -> Self {
        self.scope = scope;
        self
    }

    /// Set the rights
    pub fn with_rights(mut self, rights: Rights) -> Self {
        self.rights = rights;
        self
    }

    /// Set the quota
    pub fn with_quota(mut self, quota: Quota) -> Self {
        self.quota = quota;
        self
    }

    /// Set the expiry
    pub fn with_expires_at(mut self, expires_at: u64) -> Self {
        self.expires_at = Some(expires_at);
        self
    }
}

impl Default for DeriveRequest {
    fn default() -> Self {
        Self::new()
    }
}

/// Error during capability derivation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeriveError {
    /// Parent capability does not have DELEGATE right
    NoDelegateRight,
    /// Attempted to amplify rights (add rights parent doesn't have)
    AmplificationDenied { right: u32 },
    /// Attempted to expand scope beyond parent
    ScopeExpansion,
    /// Attempted to increase quota beyond remaining
    QuotaExceedsRemaining,
    /// Attempted to extend expiry beyond parent
    ExpiryExtension,
    /// Parent capability is invalid (expired, revoked, etc.)
    ParentInvalid,
    /// Scope types don't match
    ScopeMismatch,
}

impl core::fmt::Display for DeriveError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NoDelegateRight => write!(f, "parent capability lacks DELEGATE right"),
            Self::AmplificationDenied { right } => {
                write!(f, "cannot amplify right: {}", right)
            }
            Self::ScopeExpansion => write!(f, "cannot expand scope beyond parent"),
            Self::QuotaExceedsRemaining => write!(f, "quota exceeds remaining parent quota"),
            Self::ExpiryExtension => write!(f, "cannot extend expiry beyond parent"),
            Self::ParentInvalid => write!(f, "parent capability is invalid"),
            Self::ScopeMismatch => write!(f, "scope types do not match"),
        }
    }
}

/// Derive a child capability from a parent
///
/// The child capability will have:
/// - Rights: intersection of parent rights and requested rights
/// - Scope: intersection of parent scope and requested scope
/// - Quota: minimum of remaining parent quota and requested quota
/// - Expiry: minimum of parent expiry and requested expiry
pub fn derive_capability(
    parent: &Capability,
    request: &DeriveRequest,
    current_time: u64,
) -> Result<Capability, DeriveError> {
    // Check parent is valid
    if !parent.is_valid(current_time) {
        return Err(DeriveError::ParentInvalid);
    }

    // Check parent has DELEGATE right
    if !parent.rights.has(Rights::DELEGATE) {
        return Err(DeriveError::NoDelegateRight);
    }

    // Rights can only be reduced, never added
    // Check that requested rights are a subset of parent rights
    if !request.rights.is_subset_of(parent.rights) {
        let amplified = request.rights.0 & !parent.rights.0;
        return Err(DeriveError::AmplificationDenied { right: amplified });
    }
    let new_rights = parent.rights.intersect(request.rights);

    // Scope can only be narrowed
    let new_scope = parent
        .scope
        .intersect(&request.scope)
        .ok_or(DeriveError::ScopeMismatch)?;

    // Quota can only be reduced
    let remaining_invocations = parent.quota.remaining_invocations();
    let remaining_bytes = parent.quota.remaining_bytes();
    let remaining_duration = parent.quota.remaining_duration_ns();

    if request.quota.max_invocations > remaining_invocations {
        return Err(DeriveError::QuotaExceedsRemaining);
    }
    if request.quota.max_bytes > remaining_bytes {
        return Err(DeriveError::QuotaExceedsRemaining);
    }
    if request.quota.max_duration_ns > remaining_duration {
        return Err(DeriveError::QuotaExceedsRemaining);
    }

    let new_quota = Quota {
        max_invocations: request.quota.max_invocations.min(remaining_invocations),
        used_invocations: 0,
        max_bytes: request.quota.max_bytes.min(remaining_bytes),
        used_bytes: 0,
        max_duration_ns: request.quota.max_duration_ns.min(remaining_duration),
        used_duration_ns: 0,
    };

    // Expiry can only be sooner
    let new_expires = match request.expires_at {
        Some(requested) => {
            if requested > parent.expires_at {
                return Err(DeriveError::ExpiryExtension);
            }
            requested.min(parent.expires_at)
        }
        None => parent.expires_at,
    };

    // Create new capability ID
    let new_id = CapabilityId::generate();

    // Create proof (would sign in real implementation)
    let new_proof = CapabilityProof::new(
        parent.proof.issuer,
        [1u8; 64], // Would be a real signature
        current_time,
    );

    Ok(Capability {
        id: new_id,
        cap_type: parent.cap_type,
        scope: new_scope,
        rights: new_rights,
        quota: new_quota,
        expires_at: new_expires,
        parent: Some(parent.id),
        proof: new_proof,
        revoked: false,
    })
}

/// Check if derivation would succeed without actually creating the capability
pub fn can_derive(
    parent: &Capability,
    request: &DeriveRequest,
    current_time: u64,
) -> Result<(), DeriveError> {
    // Just run the same checks without creating the capability
    if !parent.is_valid(current_time) {
        return Err(DeriveError::ParentInvalid);
    }

    if !parent.rights.has(Rights::DELEGATE) {
        return Err(DeriveError::NoDelegateRight);
    }

    if !request.rights.is_subset_of(parent.rights) {
        let amplified = request.rights.0 & !parent.rights.0;
        return Err(DeriveError::AmplificationDenied { right: amplified });
    }

    if parent.scope.intersect(&request.scope).is_none() {
        return Err(DeriveError::ScopeMismatch);
    }

    let remaining_invocations = parent.quota.remaining_invocations();
    let remaining_bytes = parent.quota.remaining_bytes();
    let remaining_duration = parent.quota.remaining_duration_ns();

    if request.quota.max_invocations > remaining_invocations
        || request.quota.max_bytes > remaining_bytes
        || request.quota.max_duration_ns > remaining_duration
    {
        return Err(DeriveError::QuotaExceedsRemaining);
    }

    if let Some(requested) = request.expires_at {
        if requested > parent.expires_at {
            return Err(DeriveError::ExpiryExtension);
        }
    }

    Ok(())
}
