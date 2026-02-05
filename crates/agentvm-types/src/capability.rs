//! Capability types for access control

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// Unique identifier for a capability (128-bit)
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapabilityId(pub u128);

impl CapabilityId {
    /// Create a new random CapabilityId
    pub fn generate() -> Self {
        #[cfg(feature = "std")]
        {
            use std::time::{SystemTime, UNIX_EPOCH};
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            Self(nanos)
        }
        #[cfg(not(feature = "std"))]
        {
            Self(0)
        }
    }

    /// Create from raw value
    pub const fn from_raw(value: u128) -> Self {
        Self(value)
    }

    /// Get the underlying value
    pub const fn as_raw(&self) -> u128 {
        self.0
    }

    /// Create a null capability ID
    pub const fn null() -> Self {
        Self(0)
    }

    /// Check if this is a null ID
    pub fn is_null(&self) -> bool {
        self.0 == 0
    }
}

impl fmt::Debug for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CapabilityId({:#018x})", self.0 as u64)
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:032x}", self.0)
    }
}

/// Core capability type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u16)]
pub enum CapabilityType {
    // Network capabilities (0x01xx)
    NetworkHttp = 0x0100,
    NetworkTcp = 0x0101,
    NetworkDns = 0x0102,

    // Filesystem capabilities (0x02xx)
    FileRead = 0x0200,
    FileWrite = 0x0201,
    FileDelete = 0x0202,
    DirectoryList = 0x0203,

    // Process capabilities (0x03xx)
    ProcessSpawn = 0x0300,
    ProcessSignal = 0x0301,

    // Secret capabilities (0x04xx)
    SecretRead = 0x0400,
    SecretRotate = 0x0401,

    // Clock capabilities (0x05xx)
    ClockRead = 0x0500,
    ClockMonotonic = 0x0501,

    // Random capabilities (0x06xx)
    RandomSecure = 0x0600,
    RandomInsecure = 0x0601,

    // Evidence capabilities (0x07xx)
    EvidenceAppend = 0x0700,
    EvidenceRead = 0x0701,
}

impl CapabilityType {
    /// Get the type category (upper byte)
    pub fn category(&self) -> u8 {
        ((*self as u16) >> 8) as u8
    }

    /// Check if this is a network capability
    pub fn is_network(&self) -> bool {
        self.category() == 0x01
    }

    /// Check if this is a filesystem capability
    pub fn is_filesystem(&self) -> bool {
        self.category() == 0x02
    }

    /// Check if this is a process capability
    pub fn is_process(&self) -> bool {
        self.category() == 0x03
    }
}

/// Rights bit vector (32 bits)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rights(pub u32);

impl Rights {
    pub const NONE: u32 = 0;
    pub const READ: u32 = 1 << 0;
    pub const WRITE: u32 = 1 << 1;
    pub const EXECUTE: u32 = 1 << 2;
    pub const DELETE: u32 = 1 << 3;
    pub const DELEGATE: u32 = 1 << 4;
    pub const AUDIT: u32 = 1 << 5;
    pub const ALL: u32 = Self::READ | Self::WRITE | Self::EXECUTE | Self::DELETE | Self::DELEGATE | Self::AUDIT;

    /// Create new rights with the given flags
    pub const fn new(flags: u32) -> Self {
        Self(flags)
    }

    /// Create rights with all permissions
    pub const fn all() -> Self {
        Self(Self::ALL)
    }

    /// Create rights with no permissions
    pub const fn none() -> Self {
        Self(Self::NONE)
    }

    /// Check if a specific right is set
    pub const fn has(&self, right: u32) -> bool {
        (self.0 & right) == right
    }

    /// Add a right
    pub fn add(&mut self, right: u32) {
        self.0 |= right;
    }

    /// Remove a right
    pub fn remove(&mut self, right: u32) {
        self.0 &= !right;
    }

    /// Intersect with another rights set (keep only common rights)
    pub const fn intersect(&self, other: Rights) -> Rights {
        Rights(self.0 & other.0)
    }

    /// Union with another rights set
    pub const fn union(&self, other: Rights) -> Rights {
        Rights(self.0 | other.0)
    }

    /// Check if this is a subset of another rights set
    pub const fn is_subset_of(&self, other: Rights) -> bool {
        (self.0 & other.0) == self.0
    }

    /// Check if this is empty
    pub const fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Count the number of rights set
    pub const fn count(&self) -> u32 {
        self.0.count_ones()
    }
}

impl core::ops::BitOr for Rights {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl core::ops::BitAnd for Rights {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        self.intersect(rhs)
    }
}

impl core::ops::BitOrAssign for Rights {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl core::ops::BitAndAssign for Rights {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}

impl core::ops::Not for Rights {
    type Output = Self;
    fn not(self) -> Self::Output {
        Rights(!self.0)
    }
}

/// Quota tracking for capability usage
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Quota {
    /// Maximum invocations
    pub max_invocations: u64,
    pub used_invocations: u64,

    /// Maximum bytes transferred
    pub max_bytes: u64,
    pub used_bytes: u64,

    /// Maximum wall clock time (nanoseconds)
    pub max_duration_ns: u64,
    pub used_duration_ns: u64,
}

impl Quota {
    /// Create a new quota with limits
    pub fn new(max_invocations: u64, max_bytes: u64, max_duration_ns: u64) -> Self {
        Self {
            max_invocations,
            used_invocations: 0,
            max_bytes,
            used_bytes: 0,
            max_duration_ns,
            used_duration_ns: 0,
        }
    }

    /// Create an unlimited quota
    pub fn unlimited() -> Self {
        Self {
            max_invocations: u64::MAX,
            used_invocations: 0,
            max_bytes: u64::MAX,
            used_bytes: 0,
            max_duration_ns: u64::MAX,
            used_duration_ns: 0,
        }
    }

    /// Check if the quota is exhausted
    pub fn is_exhausted(&self) -> bool {
        self.used_invocations >= self.max_invocations
            || self.used_bytes >= self.max_bytes
            || self.used_duration_ns >= self.max_duration_ns
    }

    /// Get remaining invocations
    pub fn remaining_invocations(&self) -> u64 {
        self.max_invocations.saturating_sub(self.used_invocations)
    }

    /// Get remaining bytes
    pub fn remaining_bytes(&self) -> u64 {
        self.max_bytes.saturating_sub(self.used_bytes)
    }

    /// Get remaining duration in nanoseconds
    pub fn remaining_duration_ns(&self) -> u64 {
        self.max_duration_ns.saturating_sub(self.used_duration_ns)
    }

    /// Consume quota for an invocation
    pub fn consume(&mut self, bytes: u64, duration_ns: u64) -> Result<(), QuotaExceededError> {
        if self.used_invocations >= self.max_invocations {
            return Err(QuotaExceededError::Invocations);
        }
        if self.used_bytes.saturating_add(bytes) > self.max_bytes {
            return Err(QuotaExceededError::Bytes);
        }
        if self.used_duration_ns.saturating_add(duration_ns) > self.max_duration_ns {
            return Err(QuotaExceededError::Duration);
        }

        self.used_invocations += 1;
        self.used_bytes += bytes;
        self.used_duration_ns += duration_ns;
        Ok(())
    }

    /// Calculate utilization as a percentage (0-100)
    pub fn utilization(&self) -> u8 {
        let inv_util = if self.max_invocations > 0 {
            (self.used_invocations * 100 / self.max_invocations) as u8
        } else {
            0
        };
        let byte_util = if self.max_bytes > 0 {
            (self.used_bytes * 100 / self.max_bytes) as u8
        } else {
            0
        };
        let dur_util = if self.max_duration_ns > 0 {
            (self.used_duration_ns * 100 / self.max_duration_ns) as u8
        } else {
            0
        };
        inv_util.max(byte_util).max(dur_util)
    }
}

/// Error when quota is exceeded
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaExceededError {
    Invocations,
    Bytes,
    Duration,
}

/// Scope restricts what a capability can access
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CapabilityScope {
    /// Network: specific hosts/ports
    Network {
        hosts: Vec<String>,
        ports: Vec<u16>,
        protocols: Vec<Protocol>,
    },

    /// Filesystem: path patterns
    Filesystem {
        paths: Vec<String>,
        operations: FileOperations,
    },

    /// Process: allowed executables
    Process {
        executables: Vec<String>,
        args_pattern: Option<String>,
        env_allowlist: Vec<String>,
    },

    /// Secrets: named secrets
    Secrets { names: Vec<String> },

    /// Global scope (unrestricted within capability type)
    Global,
}

impl CapabilityScope {
    /// Check if this scope permits an operation on the given target
    pub fn permits(&self, target: &str) -> bool {
        match self {
            CapabilityScope::Network { hosts, .. } => {
                hosts.iter().any(|h| target.contains(h) || h == "*")
            }
            CapabilityScope::Filesystem { paths, .. } => {
                paths.iter().any(|p| target.starts_with(p) || p.ends_with("**"))
            }
            CapabilityScope::Process { executables, .. } => {
                executables.iter().any(|e| target == e || e == "*")
            }
            CapabilityScope::Secrets { names } => names.iter().any(|n| target == n),
            CapabilityScope::Global => true,
        }
    }

    /// Intersect this scope with another (narrower scope)
    pub fn intersect(&self, other: &CapabilityScope) -> Option<CapabilityScope> {
        match (self, other) {
            (CapabilityScope::Global, other) => Some(other.clone()),
            (this, CapabilityScope::Global) => Some(this.clone()),
            (
                CapabilityScope::Network {
                    hosts: h1,
                    ports: p1,
                    protocols: pr1,
                },
                CapabilityScope::Network {
                    hosts: h2,
                    ports: p2,
                    protocols: pr2,
                },
            ) => {
                let hosts: Vec<_> = h1.iter().filter(|h| h2.contains(h)).cloned().collect();
                let ports: Vec<_> = p1.iter().filter(|p| p2.contains(p)).cloned().collect();
                let protocols: Vec<_> = pr1.iter().filter(|p| pr2.contains(p)).cloned().collect();
                if hosts.is_empty() && !h1.is_empty() && !h2.is_empty() {
                    None
                } else {
                    Some(CapabilityScope::Network {
                        hosts,
                        ports,
                        protocols,
                    })
                }
            }
            (
                CapabilityScope::Filesystem { paths: p1, operations: o1 },
                CapabilityScope::Filesystem { paths: p2, operations: o2 },
            ) => {
                let paths: Vec<_> = p1.iter().filter(|p| p2.contains(p)).cloned().collect();
                let operations = o1.intersect(*o2);
                Some(CapabilityScope::Filesystem { paths, operations })
            }
            _ => None, // Cannot intersect different scope types
        }
    }
}

/// Network protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Protocol {
    Http,
    Https,
    Tcp,
    Udp,
}

/// File operation flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FileOperations(pub u8);

impl FileOperations {
    pub const READ: u8 = 1 << 0;
    pub const WRITE: u8 = 1 << 1;
    pub const DELETE: u8 = 1 << 2;
    pub const CREATE: u8 = 1 << 3;

    pub const fn new(flags: u8) -> Self {
        Self(flags)
    }

    pub const fn all() -> Self {
        Self(Self::READ | Self::WRITE | Self::DELETE | Self::CREATE)
    }

    pub const fn read_only() -> Self {
        Self(Self::READ)
    }

    pub const fn has(&self, op: u8) -> bool {
        (self.0 & op) == op
    }

    pub const fn intersect(&self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}

/// Cryptographic proof of capability issuance
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapabilityProof {
    /// Issuer public key
    pub issuer: [u8; 32],
    /// Signature over capability data
    pub signature: [u8; 64],
    /// Timestamp of issuance (ns since epoch)
    pub issued_at: u64,
}

impl CapabilityProof {
    /// Create a new proof with the given signature
    pub fn new(issuer: [u8; 32], signature: [u8; 64], issued_at: u64) -> Self {
        Self {
            issuer,
            signature,
            issued_at,
        }
    }

    /// Verify the proof against a capability
    pub fn verify(&self, _cap: &Capability) -> bool {
        // In real implementation, this would verify the Ed25519 signature
        // For now, just check signature is not all zeros
        self.signature != [0u8; 64]
    }
}

/// A capability token granting specific rights
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Capability {
    /// Unique identifier for this capability instance
    pub id: CapabilityId,

    /// Type of capability
    pub cap_type: CapabilityType,

    /// Scope restricting what the capability can access
    pub scope: CapabilityScope,

    /// Rights bit vector (type-specific)
    pub rights: Rights,

    /// Quota limits for this capability
    pub quota: Quota,

    /// When the capability expires (monotonic ns)
    pub expires_at: u64,

    /// Parent capability this was derived from (if any)
    pub parent: Option<CapabilityId>,

    /// Cryptographic proof of issuance
    pub proof: CapabilityProof,

    /// Whether this capability has been revoked
    pub revoked: bool,
}

impl Capability {
    /// Check if the capability has expired
    pub fn is_expired(&self, current_time: u64) -> bool {
        current_time >= self.expires_at
    }

    /// Check if the capability is revoked
    pub fn is_revoked(&self) -> bool {
        self.revoked
    }

    /// Check if the capability is valid for use
    pub fn is_valid(&self, current_time: u64) -> bool {
        !self.is_expired(current_time) && !self.is_revoked() && !self.quota.is_exhausted()
    }
}

/// Grant specification for creating capabilities
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapabilityGrant {
    pub cap_type: CapabilityType,
    pub scope: CapabilityScope,
    pub rights: Rights,
    pub quota: Quota,
    pub lease_secs: u64,
}

impl CapabilityGrant {
    pub fn new(cap_type: CapabilityType) -> Self {
        Self {
            cap_type,
            scope: CapabilityScope::Global,
            rights: Rights::all(),
            quota: Quota::unlimited(),
            lease_secs: 3600, // 1 hour default
        }
    }

    pub fn with_scope(mut self, scope: CapabilityScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn with_rights(mut self, rights: Rights) -> Self {
        self.rights = rights;
        self
    }

    pub fn with_quota(mut self, quota: Quota) -> Self {
        self.quota = quota;
        self
    }

    pub fn with_lease(mut self, secs: u64) -> Self {
        self.lease_secs = secs;
        self
    }
}
