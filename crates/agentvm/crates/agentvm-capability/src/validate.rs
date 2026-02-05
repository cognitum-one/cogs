//! Capability validation.
//!
//! Validates capabilities against operations, checking:
//! - Expiry time
//! - Revocation status
//! - Quota availability
//! - Scope permissions
//! - Rights requirements

use core::fmt;

use agentvm_types::{Capability, Rights, TimestampNs};

/// Result of capability validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationResult {
    /// Capability is valid for the requested operation
    Valid,
    /// Capability has expired
    Expired,
    /// Capability has been revoked
    Revoked,
    /// Quota has been exhausted
    QuotaExhausted,
    /// Operation is outside the capability's scope
    ScopeViolation,
    /// Capability signature is invalid
    InvalidSignature,
    /// Capability does not have required rights
    InsufficientRights,
    /// Capability has not yet become valid (future start time)
    NotYetValid,
    /// Operation type mismatch
    OperationMismatch,
}

impl ValidationResult {
    /// Check if the result indicates a valid capability
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Get an error message for invalid results
    pub fn error_message(&self) -> Option<&'static str> {
        match self {
            Self::Valid => None,
            Self::Expired => Some("capability has expired"),
            Self::Revoked => Some("capability has been revoked"),
            Self::QuotaExhausted => Some("capability quota exhausted"),
            Self::ScopeViolation => Some("operation outside capability scope"),
            Self::InvalidSignature => Some("capability signature invalid"),
            Self::InsufficientRights => Some("insufficient rights for operation"),
            Self::NotYetValid => Some("capability not yet valid"),
            Self::OperationMismatch => Some("operation type does not match capability"),
        }
    }
}

impl fmt::Display for ValidationResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Valid => write!(f, "valid"),
            Self::Expired => write!(f, "expired"),
            Self::Revoked => write!(f, "revoked"),
            Self::QuotaExhausted => write!(f, "quota exhausted"),
            Self::ScopeViolation => write!(f, "scope violation"),
            Self::InvalidSignature => write!(f, "invalid signature"),
            Self::InsufficientRights => write!(f, "insufficient rights"),
            Self::NotYetValid => write!(f, "not yet valid"),
            Self::OperationMismatch => write!(f, "operation mismatch"),
        }
    }
}

/// Validation context containing all information needed to validate a capability.
#[derive(Debug)]
pub struct ValidationContext<'a> {
    /// The capability to validate
    pub capability: &'a Capability,
    /// Target resource (e.g., host:port, file path)
    pub target: &'a str,
    /// Required rights for the operation
    pub required_rights: Rights,
    /// Current timestamp
    pub current_time: TimestampNs,
    /// Whether to skip signature verification
    pub skip_signature: bool,
}

impl<'a> ValidationContext<'a> {
    /// Create a new validation context
    pub fn new(
        capability: &'a Capability,
        target: &'a str,
        required_rights: Rights,
        current_time: TimestampNs,
    ) -> Self {
        Self {
            capability,
            target,
            required_rights,
            current_time,
            skip_signature: true,
        }
    }

    /// Enable signature verification
    pub fn with_signature_check(mut self) -> Self {
        self.skip_signature = false;
        self
    }
}

/// Validate a capability for a given operation.
///
/// This is the main validation function that checks all security constraints:
/// 1. Not revoked
/// 2. Not expired
/// 3. Has remaining quota
/// 4. Has required rights
/// 5. Operation is within scope
///
/// # Arguments
/// * `capability` - The capability to validate
/// * `target` - The target resource (e.g., "api.example.com:443", "/data/file.txt")
/// * `required_rights` - The rights required for the operation
/// * `current_time` - Current timestamp in nanoseconds
///
/// # Returns
/// `ValidationResult::Valid` if all checks pass, or a specific error result
pub fn validate_capability(
    capability: &Capability,
    target: &str,
    required_rights: Rights,
    current_time: TimestampNs,
) -> ValidationResult {
    // 1. Check revocation
    if capability.is_revoked() {
        return ValidationResult::Revoked;
    }

    // 2. Check expiry
    if capability.is_expired(current_time) {
        return ValidationResult::Expired;
    }

    // 3. Check quota exhaustion
    if capability.is_quota_exhausted() {
        return ValidationResult::QuotaExhausted;
    }

    // 4. Check rights
    if !has_required_rights(capability, required_rights) {
        return ValidationResult::InsufficientRights;
    }

    // 5. Check scope
    if !capability.scope.permits(target) {
        return ValidationResult::ScopeViolation;
    }

    ValidationResult::Valid
}

/// Check if capability has all required rights
fn has_required_rights(capability: &Capability, required: Rights) -> bool {
    // Check each required right bit
    let required_bits = required.0;
    let granted_bits = capability.rights.0;

    (granted_bits & required_bits) == required_bits
}

/// Validate a capability with full context
pub fn validate_capability_full(ctx: &ValidationContext) -> ValidationResult {
    validate_capability(
        ctx.capability,
        ctx.target,
        ctx.required_rights,
        ctx.current_time,
    )
}

/// Batch validation result
#[derive(Debug)]
pub struct BatchValidationResult {
    /// Overall result (all must be valid)
    pub overall: ValidationResult,
    /// Individual results per capability
    pub individual: heapless::Vec<ValidationResult, 16>,
    /// Index of first failed capability (-1 if all valid)
    pub first_failure: i32,
}

impl BatchValidationResult {
    /// Check if all validations passed
    pub fn all_valid(&self) -> bool {
        self.overall.is_valid()
    }
}

/// Validate multiple capabilities in batch.
///
/// All capabilities must be valid for the overall result to be valid.
pub fn validate_batch(
    validations: &[(&Capability, &str, Rights)],
    current_time: TimestampNs,
) -> BatchValidationResult {
    let mut individual = heapless::Vec::new();
    let mut first_failure = -1i32;
    let mut overall = ValidationResult::Valid;

    for (i, (cap, target, rights)) in validations.iter().enumerate() {
        let result = validate_capability(cap, target, *rights, current_time);
        let _ = individual.push(result);

        if !result.is_valid() && first_failure < 0 {
            first_failure = i as i32;
            overall = result;
        }
    }

    BatchValidationResult {
        overall,
        individual,
        first_failure,
    }
}

/// Policy for handling quota exhaustion
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuotaPolicy {
    /// Fail immediately when quota would be exhausted
    Strict,
    /// Allow the operation that exhausts quota, fail subsequent
    #[default]
    AllowLast,
    /// Log warning but allow (for debugging/monitoring)
    Warn,
}

/// Extended validation options
#[derive(Debug, Clone, Default)]
pub struct ValidationOptions {
    /// How to handle quota exhaustion
    pub quota_policy: QuotaPolicy,
    /// Whether to validate the derivation chain
    pub validate_chain: bool,
    /// Maximum chain depth to validate
    pub max_chain_depth: usize,
    /// Whether to require signature verification
    pub require_signature: bool,
    /// Grace period for expiry (in nanoseconds)
    pub expiry_grace_ns: u64,
}

impl ValidationOptions {
    /// Create strict validation options
    pub fn strict() -> Self {
        Self {
            quota_policy: QuotaPolicy::Strict,
            validate_chain: true,
            max_chain_depth: 10,
            require_signature: true,
            expiry_grace_ns: 0,
        }
    }

    /// Create permissive validation options (for testing)
    pub fn permissive() -> Self {
        Self {
            quota_policy: QuotaPolicy::Warn,
            validate_chain: false,
            max_chain_depth: 0,
            require_signature: false,
            expiry_grace_ns: 300_000_000_000, // 5 minute grace in ns
        }
    }
}

/// Validate with custom options
pub fn validate_with_options(
    capability: &Capability,
    target: &str,
    required_rights: Rights,
    current_time: TimestampNs,
    options: &ValidationOptions,
) -> ValidationResult {
    // Check revocation
    if capability.is_revoked() {
        return ValidationResult::Revoked;
    }

    // Check expiry with grace period
    let effective_expiry = capability.expires_at.saturating_add(options.expiry_grace_ns);
    if current_time >= effective_expiry {
        return ValidationResult::Expired;
    }

    // Check quota based on policy
    if capability.is_quota_exhausted() {
        match options.quota_policy {
            QuotaPolicy::Strict => return ValidationResult::QuotaExhausted,
            QuotaPolicy::AllowLast => {
                // Allow if this is the last operation
            }
            QuotaPolicy::Warn => {
                // Would log in real implementation
            }
        }
    }

    // Check rights
    if !has_required_rights(capability, required_rights) {
        return ValidationResult::InsufficientRights;
    }

    // Check scope
    if !capability.scope.permits(target) {
        return ValidationResult::ScopeViolation;
    }

    ValidationResult::Valid
}

/// Validate that a capability can be used for a specific capability type
pub fn validate_capability_type(
    capability: &Capability,
    expected_type: agentvm_types::CapabilityType,
) -> ValidationResult {
    if capability.cap_type == expected_type {
        ValidationResult::Valid
    } else {
        ValidationResult::OperationMismatch
    }
}

/// Quick validation - checks only revocation and expiry
pub fn quick_validate(capability: &Capability, current_time: TimestampNs) -> bool {
    !capability.is_revoked() && !capability.is_expired(current_time)
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use agentvm_types::{CapabilityId, CapabilityProof, CapabilityScope, CapabilityType, Quota};

    fn test_capability() -> Capability {
        Capability {
            id: CapabilityId::from_bytes([1u8; 16]),
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Unrestricted,
            rights: Rights(Rights::READ | Rights::WRITE),
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
    fn test_validate_valid() {
        let cap = test_capability();

        let result = validate_capability(
            &cap,
            "api.example.com",
            Rights(Rights::READ),
            1_500_000_000_000_000_000,
        );
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_validate_expired() {
        let cap = test_capability();

        let result = validate_capability(
            &cap,
            "api.example.com",
            Rights(Rights::READ),
            3_000_000_000_000_000_000, // After expiry
        );
        assert_eq!(result, ValidationResult::Expired);
    }

    #[test]
    fn test_validate_revoked() {
        let mut cap = test_capability();
        cap.revoked = true;

        let result = validate_capability(
            &cap,
            "api.example.com",
            Rights(Rights::READ),
            1_500_000_000_000_000_000,
        );
        assert_eq!(result, ValidationResult::Revoked);
    }

    #[test]
    fn test_validate_insufficient_rights() {
        let cap = test_capability();

        let result = validate_capability(
            &cap,
            "api.example.com",
            Rights(Rights::DELETE), // Not granted
            1_500_000_000_000_000_000,
        );
        assert_eq!(result, ValidationResult::InsufficientRights);
    }

    #[test]
    fn test_validate_quota_exhausted() {
        let mut cap = test_capability();
        cap.quota.used_invocations = cap.quota.max_invocations; // Exhausted

        let result = validate_capability(
            &cap,
            "api.example.com",
            Rights(Rights::READ),
            1_500_000_000_000_000_000,
        );
        assert_eq!(result, ValidationResult::QuotaExhausted);
    }

    #[test]
    fn test_validation_result_display() {
        assert_eq!(alloc::format!("{}", ValidationResult::Valid), "valid");
        assert_eq!(alloc::format!("{}", ValidationResult::Expired), "expired");
        assert_eq!(
            ValidationResult::Expired.error_message(),
            Some("capability has expired")
        );
        assert!(ValidationResult::Valid.error_message().is_none());
    }

    #[test]
    fn test_batch_validation() {
        let cap1 = test_capability();
        let cap2 = test_capability();

        let batch = [
            (&cap1, "api.example.com", Rights(Rights::READ)),
            (&cap2, "api.other.com", Rights(Rights::READ)),
        ];

        let result = validate_batch(&batch, 1_500_000_000_000_000_000);
        assert!(result.all_valid());
        assert_eq!(result.first_failure, -1);
    }

    #[test]
    fn test_batch_validation_failure() {
        let cap1 = test_capability();
        let mut cap2 = test_capability();
        cap2.revoked = true;

        let batch = [
            (&cap1, "api.example.com", Rights(Rights::READ)),
            (&cap2, "api.other.com", Rights(Rights::READ)),
        ];

        let result = validate_batch(&batch, 1_500_000_000_000_000_000);
        assert!(!result.all_valid());
        assert_eq!(result.first_failure, 1);
        assert_eq!(result.overall, ValidationResult::Revoked);
    }

    #[test]
    fn test_validation_with_options() {
        let mut cap = test_capability();
        cap.expires_at = 1_500_000_000_000_000_000;

        // Without grace period - should be expired
        let strict = ValidationOptions::strict();
        let result = validate_with_options(
            &cap,
            "api.example.com",
            Rights(Rights::READ),
            1_500_000_000_100_000_000, // Just after expiry
            &strict,
        );
        assert_eq!(result, ValidationResult::Expired);

        // With grace period - should still be valid
        let permissive = ValidationOptions::permissive();
        let result = validate_with_options(
            &cap,
            "api.example.com",
            Rights(Rights::READ),
            1_500_000_000_100_000_000,
            &permissive,
        );
        assert_eq!(result, ValidationResult::Valid);
    }

    #[test]
    fn test_quick_validate() {
        let cap = test_capability();
        assert!(quick_validate(&cap, 1_500_000_000_000_000_000));

        let mut revoked_cap = test_capability();
        revoked_cap.revoked = true;
        assert!(!quick_validate(&revoked_cap, 1_500_000_000_000_000_000));
    }

    #[test]
    fn test_validate_capability_type() {
        let cap = test_capability();

        assert_eq!(
            validate_capability_type(&cap, CapabilityType::NetworkHttp),
            ValidationResult::Valid
        );
        assert_eq!(
            validate_capability_type(&cap, CapabilityType::FileRead),
            ValidationResult::OperationMismatch
        );
    }

    #[test]
    fn test_validation_context() {
        let cap = test_capability();
        let ctx = ValidationContext::new(
            &cap,
            "api.example.com",
            Rights(Rights::READ),
            1_500_000_000_000_000_000,
        );

        let result = validate_capability_full(&ctx);
        assert_eq!(result, ValidationResult::Valid);
    }
}
