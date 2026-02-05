//! Capability types - tokens granting specific rights to agent capsules

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use crate::{Hash, TimestampNs};

/// Unique identifier for a capability (128-bit random)
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapabilityId(pub [u8; 16]);

impl CapabilityId {
    /// Generate a new random capability ID
    pub fn generate() -> Self {
        static COUNTER: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(1000);
        let count = COUNTER.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&count.to_le_bytes());
        let ptr = &bytes as *const _ as u64;
        bytes[8..].copy_from_slice(&ptr.wrapping_mul(0x517cc1b727220a95).to_le_bytes());
        Self(bytes)
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}

impl fmt::Debug for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CapabilityId(")?;
        for byte in &self.0[..4] {
            write!(f, "{:02x}", byte)?;
        }
        write!(f, "...)")
    }
}

/// Type of capability (what kind of resource access it grants)
///
/// Each capability type represents a class of external effects that can be
/// audited and controlled independently. Types are organized by category:
/// - Network (0x01xx): HTTP, TCP, DNS, WebSocket, UDP
/// - Filesystem (0x02xx): Read, Write, Delete, List, Create, Watch
/// - Process (0x03xx): Spawn, Signal, Env
/// - Secrets (0x04xx): Read, Rotate, Write
/// - Clock (0x05xx): Wall clock, Monotonic
/// - Random (0x06xx): Secure, Insecure
/// - Evidence (0x07xx): Append, Read
/// - Inter-capsule (0x08xx): Send, Receive
/// - Resource (0x09xx): Memory, GPU
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u16)]
pub enum CapabilityType {
    // Network capabilities (0x01xx)
    /// HTTP/HTTPS requests
    NetworkHttp = 0x0100,
    /// Raw TCP connections
    NetworkTcp = 0x0101,
    /// DNS resolution
    NetworkDns = 0x0102,
    /// WebSocket connections
    NetworkWebSocket = 0x0103,
    /// UDP sockets
    NetworkUdp = 0x0104,

    // Filesystem capabilities (0x02xx)
    /// Read files
    FileRead = 0x0200,
    /// Write files
    FileWrite = 0x0201,
    /// Delete files
    FileDelete = 0x0202,
    /// List directories
    DirectoryList = 0x0203,
    /// Create directories
    DirectoryCreate = 0x0204,
    /// Watch filesystem events
    FileWatch = 0x0205,

    // Process capabilities (0x03xx)
    /// Spawn subprocesses
    ProcessSpawn = 0x0300,
    /// Send signals to processes
    ProcessSignal = 0x0301,
    /// Read process environment
    ProcessEnvRead = 0x0302,

    // Secret capabilities (0x04xx)
    /// Read secrets
    SecretRead = 0x0400,
    /// Rotate secrets
    SecretRotate = 0x0401,
    /// Write/create secrets
    SecretWrite = 0x0402,

    // Clock capabilities (0x05xx)
    /// Read wall clock time
    ClockRead = 0x0500,
    /// Read monotonic time
    ClockMonotonic = 0x0501,

    // Random capabilities (0x06xx)
    /// Cryptographically secure random
    RandomSecure = 0x0600,
    /// Fast pseudo-random
    RandomInsecure = 0x0601,

    // Evidence capabilities (0x07xx)
    /// Append to evidence log
    EvidenceAppend = 0x0700,
    /// Read own evidence
    EvidenceRead = 0x0701,

    // Inter-capsule capabilities (0x08xx)
    /// Send message to another capsule
    CapsuleSend = 0x0800,
    /// Receive message from another capsule
    CapsuleReceive = 0x0801,

    // Resource capabilities (0x09xx)
    /// Allocate memory
    MemoryAllocate = 0x0900,
    /// GPU compute access
    GpuCompute = 0x0901,
}

impl CapabilityType {
    /// Returns the scope type this capability belongs to
    pub const fn scope_type(&self) -> CapabilityScopeType {
        match self {
            Self::NetworkHttp
            | Self::NetworkTcp
            | Self::NetworkDns
            | Self::NetworkWebSocket
            | Self::NetworkUdp => CapabilityScopeType::Network,

            Self::FileRead
            | Self::FileWrite
            | Self::FileDelete
            | Self::DirectoryList
            | Self::DirectoryCreate
            | Self::FileWatch => CapabilityScopeType::Filesystem,

            Self::ProcessSpawn | Self::ProcessSignal | Self::ProcessEnvRead => {
                CapabilityScopeType::Process
            }

            Self::SecretRead | Self::SecretRotate | Self::SecretWrite => {
                CapabilityScopeType::Secrets
            }

            Self::ClockRead | Self::ClockMonotonic => CapabilityScopeType::Clock,

            Self::RandomSecure | Self::RandomInsecure => CapabilityScopeType::Random,

            Self::EvidenceAppend | Self::EvidenceRead => CapabilityScopeType::Evidence,

            Self::CapsuleSend | Self::CapsuleReceive => CapabilityScopeType::InterCapsule,

            Self::MemoryAllocate | Self::GpuCompute => CapabilityScopeType::Resource,
        }
    }

    /// Returns a human-readable name for this capability type
    pub const fn name(&self) -> &'static str {
        match self {
            Self::NetworkHttp => "network.http",
            Self::NetworkTcp => "network.tcp",
            Self::NetworkDns => "network.dns",
            Self::NetworkWebSocket => "network.websocket",
            Self::NetworkUdp => "network.udp",
            Self::FileRead => "filesystem.read",
            Self::FileWrite => "filesystem.write",
            Self::FileDelete => "filesystem.delete",
            Self::DirectoryList => "filesystem.list",
            Self::DirectoryCreate => "filesystem.mkdir",
            Self::FileWatch => "filesystem.watch",
            Self::ProcessSpawn => "process.spawn",
            Self::ProcessSignal => "process.signal",
            Self::ProcessEnvRead => "process.env",
            Self::SecretRead => "secrets.read",
            Self::SecretRotate => "secrets.rotate",
            Self::SecretWrite => "secrets.write",
            Self::ClockRead => "clock.read",
            Self::ClockMonotonic => "clock.monotonic",
            Self::RandomSecure => "random.secure",
            Self::RandomInsecure => "random.insecure",
            Self::EvidenceAppend => "evidence.append",
            Self::EvidenceRead => "evidence.read",
            Self::CapsuleSend => "capsule.send",
            Self::CapsuleReceive => "capsule.receive",
            Self::MemoryAllocate => "resource.memory",
            Self::GpuCompute => "resource.gpu",
        }
    }
}

/// High-level scope categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum CapabilityScopeType {
    /// Network-related capabilities
    Network = 0,
    /// Filesystem-related capabilities
    Filesystem = 1,
    /// Process-related capabilities
    Process = 2,
    /// Secret-related capabilities
    Secrets = 3,
    /// Clock-related capabilities
    Clock = 4,
    /// Random number capabilities
    Random = 5,
    /// Evidence-related capabilities
    Evidence = 6,
    /// Inter-capsule communication
    InterCapsule = 7,
    /// Resource allocation
    Resource = 8,
}

/// A capability token granting specific rights
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Capability {
    /// Unique identifier
    pub id: CapabilityId,
    /// Type of capability
    pub cap_type: CapabilityType,
    /// Scope (what resources this capability can access)
    pub scope: CapabilityScope,
    /// Rights bit vector
    pub rights: Rights,
    /// Quota limits
    pub quota: Quota,
    /// Expiration time (monotonic nanoseconds)
    pub expires_at: TimestampNs,
    /// Parent capability this was derived from
    pub parent: Option<CapabilityId>,
    /// Cryptographic proof of issuance
    pub proof: CapabilityProof,
    /// Whether this capability has been revoked
    pub revoked: bool,
}

impl Capability {
    /// Check if capability is expired
    pub fn is_expired(&self, current_time: TimestampNs) -> bool {
        current_time >= self.expires_at
    }

    /// Check if capability is revoked
    pub fn is_revoked(&self) -> bool {
        self.revoked
    }

    /// Check if quota is exhausted
    pub fn is_quota_exhausted(&self) -> bool {
        self.quota.is_exhausted()
    }
}

/// Scope restricts what a capability can access
///
/// Scopes define the boundaries of a capability's authority,
/// ensuring capabilities are narrowly tailored to specific resources.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CapabilityScope {
    /// Network scope (hosts, ports, protocols)
    Network(NetworkScope),
    /// Filesystem scope (path patterns)
    Filesystem(FilesystemScope),
    /// Process scope (allowed executables)
    Process(ProcessScope),
    /// Secrets scope (named secrets)
    Secrets(SecretsScope),
    /// Unrestricted (use with caution)
    Unrestricted,
}

impl CapabilityScope {
    /// Check if this scope permits access to the given target
    ///
    /// # Arguments
    /// * `target` - The resource target (e.g., host:port, path, executable name)
    ///
    /// # Returns
    /// `true` if access is permitted, `false` otherwise
    pub fn permits(&self, target: &str) -> bool {
        match self {
            Self::Unrestricted => true,
            Self::Network(scope) => scope.permits(target),
            Self::Filesystem(scope) => scope.permits(target),
            Self::Process(scope) => scope.permits(target),
            Self::Secrets(scope) => scope.permits(target),
        }
    }

    /// Attempt to create an intersection of two scopes (for derivation)
    ///
    /// Returns `None` if the scopes are incompatible types
    pub fn intersect(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (Self::Unrestricted, other) => Some(other.clone()),
            (_, Self::Unrestricted) => Some(self.clone()),
            (Self::Network(a), Self::Network(b)) => Some(Self::Network(a.intersect(b))),
            (Self::Filesystem(a), Self::Filesystem(b)) => Some(Self::Filesystem(a.intersect(b))),
            (Self::Process(a), Self::Process(b)) => Some(Self::Process(a.intersect(b))),
            (Self::Secrets(a), Self::Secrets(b)) => Some(Self::Secrets(a.intersect(b))),
            _ => None, // Incompatible scope types
        }
    }
}

impl Default for CapabilityScope {
    fn default() -> Self {
        Self::Unrestricted
    }
}

/// Network access scope
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NetworkScope {
    /// Allowed host patterns (e.g., "*.github.com", "api.anthropic.com")
    pub hosts: Vec<String>,
    /// Allowed port ranges
    pub ports: Vec<PortRange>,
    /// Allowed protocols
    pub protocols: Vec<Protocol>,
}

impl NetworkScope {
    /// Check if a target host is permitted
    ///
    /// Supports wildcard patterns like "*.github.com"
    pub fn permits(&self, target: &str) -> bool {
        if self.hosts.is_empty() {
            return true; // No restrictions
        }

        // Extract host from target (may include port)
        let host = target.split(':').next().unwrap_or(target);

        for pattern in &self.hosts {
            if pattern.starts_with("*.") {
                let suffix = &pattern[1..]; // ".github.com"
                if host.ends_with(suffix) || host == &pattern[2..] {
                    return true;
                }
            } else if host == pattern || target.starts_with(pattern) {
                return true;
            }
        }

        false
    }

    /// Create intersection of two network scopes
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            hosts: intersect_string_vecs(&self.hosts, &other.hosts),
            ports: intersect_port_ranges(&self.ports, &other.ports),
            protocols: intersect_protocols(&self.protocols, &other.protocols),
        }
    }
}

/// Port range
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PortRange {
    /// Start port (inclusive)
    pub start: u16,
    /// End port (inclusive)
    pub end: u16,
}

impl PortRange {
    /// Single port
    pub fn single(port: u16) -> Self {
        Self { start: port, end: port }
    }

    /// Port range
    pub fn range(start: u16, end: u16) -> Self {
        Self { start, end }
    }

    /// Check if port is in range
    pub fn contains(&self, port: u16) -> bool {
        port >= self.start && port <= self.end
    }
}

/// Network protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum Protocol {
    /// HTTP only
    Http = 1,
    /// HTTPS only
    Https = 2,
    /// Raw TCP
    Tcp = 3,
    /// UDP
    Udp = 4,
}

/// Filesystem access scope
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FilesystemScope {
    /// Allowed path patterns (glob-style)
    pub paths: Vec<PathPattern>,
}

impl FilesystemScope {
    /// Check if a path is permitted
    ///
    /// Exclusion patterns (starting with !) take precedence
    pub fn permits(&self, path: &str) -> bool {
        if self.paths.is_empty() {
            return true; // No restrictions
        }

        // Check exclusions first (they take precedence)
        for pattern in &self.paths {
            if pattern.exclude && matches_glob(&pattern.pattern, path) {
                return false;
            }
        }

        // Check inclusions
        for pattern in &self.paths {
            if !pattern.exclude && matches_glob(&pattern.pattern, path) {
                return true;
            }
        }

        false
    }

    /// Create intersection of two filesystem scopes
    pub fn intersect(&self, other: &Self) -> Self {
        // Union exclusions, intersect inclusions
        let mut combined = Vec::new();

        // Add all exclusions from both
        for pattern in self.paths.iter().chain(other.paths.iter()) {
            if pattern.exclude && !combined.iter().any(|p: &PathPattern| p.pattern == pattern.pattern) {
                combined.push(pattern.clone());
            }
        }

        // Intersect inclusions
        let self_includes: Vec<_> = self.paths.iter().filter(|p| !p.exclude).collect();
        let other_includes: Vec<_> = other.paths.iter().filter(|p| !p.exclude).collect();

        if self_includes.is_empty() {
            for p in other_includes {
                combined.push(p.clone());
            }
        } else if other_includes.is_empty() {
            for p in self_includes {
                combined.push(p.clone());
            }
        } else {
            // Take intersection of included patterns
            for p in &self_includes {
                if other_includes.iter().any(|o| o.pattern == p.pattern) {
                    combined.push((*p).clone());
                }
            }
        }

        Self { paths: combined }
    }
}

/// Path pattern for filesystem scope
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PathPattern {
    /// Pattern string (e.g., "/workspace/**", "!/workspace/.env")
    pub pattern: String,
    /// Whether this is an exclusion pattern (starts with !)
    pub exclude: bool,
}

/// Process spawn scope
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProcessScope {
    /// Allowed executable names
    pub executables: Vec<String>,
    /// Allowed environment variables
    pub env_allowlist: Vec<String>,
    /// Optional argument pattern (regex-style)
    pub args_pattern: Option<String>,
}

impl ProcessScope {
    /// Check if an executable is permitted
    pub fn permits(&self, executable: &str) -> bool {
        if self.executables.is_empty() {
            return true;
        }

        self.executables.iter().any(|e| e == executable)
    }

    /// Create intersection of two process scopes
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            executables: intersect_string_vecs(&self.executables, &other.executables),
            env_allowlist: intersect_string_vecs(&self.env_allowlist, &other.env_allowlist),
            args_pattern: self.args_pattern.clone().or_else(|| other.args_pattern.clone()),
        }
    }
}

/// Secrets access scope
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SecretsScope {
    /// Allowed secret names
    pub names: Vec<String>,
}

impl SecretsScope {
    /// Check if a secret name is permitted
    pub fn permits(&self, name: &str) -> bool {
        if self.names.is_empty() {
            return true;
        }

        self.names.iter().any(|n| n == name)
    }

    /// Create intersection of two secrets scopes
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            names: intersect_string_vecs(&self.names, &other.names),
        }
    }
}

/// Rights bit vector (what operations are allowed)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Rights(pub u32);

impl Rights {
    /// Read access
    pub const READ: u32 = 1 << 0;
    /// Write access
    pub const WRITE: u32 = 1 << 1;
    /// Execute access
    pub const EXECUTE: u32 = 1 << 2;
    /// Delete access
    pub const DELETE: u32 = 1 << 3;
    /// Can delegate (derive child capabilities)
    pub const DELEGATE: u32 = 1 << 4;
    /// Can read audit logs
    pub const AUDIT: u32 = 1 << 5;

    /// No rights
    pub const NONE: Rights = Rights(0);
    /// All rights
    pub const ALL: Rights = Rights(0x3F);

    /// Check if a specific right is granted
    pub fn has(&self, right: u32) -> bool {
        self.0 & right != 0
    }

    /// Intersect with another rights set (for derivation)
    pub fn intersect(&self, other: Rights) -> Rights {
        Rights(self.0 & other.0)
    }

    /// Check if this rights set is a subset of another
    pub fn is_subset_of(&self, other: Rights) -> bool {
        (self.0 & other.0) == self.0
    }

    /// Get the raw bits value
    pub fn bits(&self) -> u32 {
        self.0
    }

    /// Create from raw bits value
    pub fn from_bits(bits: u32) -> Self {
        Rights(bits)
    }
}

/// Quota tracking for capability usage
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Quota {
    /// Maximum invocations allowed
    pub max_invocations: u64,
    /// Invocations used
    pub used_invocations: u64,
    /// Maximum bytes allowed
    pub max_bytes: u64,
    /// Bytes used
    pub used_bytes: u64,
    /// Maximum duration allowed (nanoseconds)
    pub max_duration_ns: u64,
    /// Duration used (nanoseconds)
    pub used_duration_ns: u64,
}

impl Quota {
    /// Unlimited quota
    pub const UNLIMITED: Quota = Quota {
        max_invocations: u64::MAX,
        used_invocations: 0,
        max_bytes: u64::MAX,
        used_bytes: 0,
        max_duration_ns: u64::MAX,
        used_duration_ns: 0,
    };

    /// Check if quota is exhausted
    pub fn is_exhausted(&self) -> bool {
        self.used_invocations >= self.max_invocations
            || self.used_bytes >= self.max_bytes
            || self.used_duration_ns >= self.max_duration_ns
    }

    /// Remaining invocations
    pub fn remaining_invocations(&self) -> u64 {
        self.max_invocations.saturating_sub(self.used_invocations)
    }

    /// Remaining bytes
    pub fn remaining_bytes(&self) -> u64 {
        self.max_bytes.saturating_sub(self.used_bytes)
    }
}

impl Default for Quota {
    fn default() -> Self {
        Self::UNLIMITED
    }
}

/// Cryptographic proof of capability issuance
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapabilityProof {
    /// Issuer public key hash
    pub issuer: Hash,
    /// Signature over capability fields
    pub signature: [u8; 64],
    /// Timestamp of issuance
    pub issued_at: TimestampNs,
}

impl CapabilityProof {
    /// Serialize proof to bytes (96 bytes: 32 + 64 = 96, but we also need issued_at)
    /// Format: issuer (32) + signature (64) = 96 bytes
    pub fn to_bytes(&self) -> [u8; 96] {
        let mut bytes = [0u8; 96];
        bytes[0..32].copy_from_slice(&self.issuer);
        bytes[32..96].copy_from_slice(&self.signature);
        bytes
    }

    /// Deserialize proof from bytes
    pub fn from_bytes(bytes: &[u8; 96]) -> Self {
        let mut issuer = [0u8; 32];
        let mut signature = [0u8; 64];
        issuer.copy_from_slice(&bytes[0..32]);
        signature.copy_from_slice(&bytes[32..96]);
        Self {
            issuer,
            signature,
            issued_at: 0, // Would need more bytes to include this
        }
    }
}

/// Capability grant (used in manifests)
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CapabilityGrant {
    /// Type of capability to grant
    pub cap_type: CapabilityType,
    /// Scope of the grant
    pub scope: CapabilityScope,
    /// Rights to grant
    pub rights: Rights,
    /// Quota limits
    pub quota: Quota,
    /// Lease duration in seconds (0 = persistent)
    pub lease_secs: u64,
    /// Audit level for this capability
    pub audit_level: AuditLevel,
}

/// Audit level for capability usage
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum AuditLevel {
    /// No logging
    None = 0,
    /// Log metadata only
    Summary = 1,
    /// Full request/response logging
    Full = 2,
}

impl Default for AuditLevel {
    fn default() -> Self {
        Self::Full
    }
}

// ============================================================================
// Helper functions for scope intersection and pattern matching
// ============================================================================

/// Intersect two string vectors (return common elements, or all if one is empty)
fn intersect_string_vecs(a: &[String], b: &[String]) -> Vec<String> {
    if a.is_empty() {
        return b.to_vec();
    }
    if b.is_empty() {
        return a.to_vec();
    }

    a.iter()
        .filter(|s| b.iter().any(|t| t == *s))
        .cloned()
        .collect()
}

/// Intersect port ranges
fn intersect_port_ranges(a: &[PortRange], b: &[PortRange]) -> Vec<PortRange> {
    if a.is_empty() {
        return b.to_vec();
    }
    if b.is_empty() {
        return a.to_vec();
    }

    let mut result = Vec::new();
    for ra in a {
        for rb in b {
            // Check for overlap
            if ra.start <= rb.end && rb.start <= ra.end {
                result.push(PortRange {
                    start: ra.start.max(rb.start),
                    end: ra.end.min(rb.end),
                });
            }
        }
    }
    result
}

/// Intersect protocol lists
fn intersect_protocols(a: &[Protocol], b: &[Protocol]) -> Vec<Protocol> {
    if a.is_empty() {
        return b.to_vec();
    }
    if b.is_empty() {
        return a.to_vec();
    }

    a.iter().filter(|p| b.contains(p)).copied().collect()
}

/// Simple glob pattern matching
///
/// Supports:
/// - `**` for recursive directory matching
/// - `*` for single directory component matching
/// - Exact path matching
fn matches_glob(pattern: &str, path: &str) -> bool {
    if pattern.ends_with("/**") {
        let prefix = &pattern[..pattern.len() - 3];
        return path.starts_with(prefix);
    }
    if pattern.ends_with("/*") {
        let prefix = &pattern[..pattern.len() - 1]; // Keep the trailing /
        if !path.starts_with(prefix) {
            return false;
        }
        // Must not have additional path separators after the prefix
        let remainder = &path[prefix.len()..];
        return !remainder.contains('/');
    }
    pattern == path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_scope_permits() {
        let scope = NetworkScope {
            hosts: alloc::vec!["*.github.com".into(), "api.anthropic.com".into()],
            ports: Vec::new(),
            protocols: Vec::new(),
        };

        assert!(scope.permits("api.github.com"));
        assert!(scope.permits("raw.github.com"));
        assert!(scope.permits("api.anthropic.com"));
        assert!(!scope.permits("api.google.com"));
    }

    #[test]
    fn test_filesystem_scope_permits() {
        let scope = FilesystemScope {
            paths: alloc::vec![
                PathPattern { pattern: "/workspace/**".into(), exclude: false },
                PathPattern { pattern: "/workspace/.env".into(), exclude: true },
            ],
        };

        assert!(scope.permits("/workspace/src/main.rs"));
        assert!(scope.permits("/workspace/Cargo.toml"));
        assert!(!scope.permits("/workspace/.env")); // Excluded
        assert!(!scope.permits("/etc/passwd")); // Not in workspace
    }

    #[test]
    fn test_process_scope_permits() {
        let scope = ProcessScope {
            executables: alloc::vec!["npm".into(), "node".into(), "git".into()],
            env_allowlist: Vec::new(),
            args_pattern: None,
        };

        assert!(scope.permits("npm"));
        assert!(scope.permits("node"));
        assert!(scope.permits("git"));
        assert!(!scope.permits("rm"));
    }

    #[test]
    fn test_rights_operations() {
        let rw = Rights(Rights::READ | Rights::WRITE);
        assert!(rw.has(Rights::READ));
        assert!(rw.has(Rights::WRITE));
        assert!(!rw.has(Rights::DELETE));

        let r = Rights(Rights::READ);
        assert!(r.is_subset_of(rw));
        assert!(!rw.is_subset_of(r));
    }

    #[test]
    fn test_quota_tracking() {
        let mut quota = Quota {
            max_invocations: 10,
            used_invocations: 5,
            max_bytes: u64::MAX,
            used_bytes: 0,
            max_duration_ns: u64::MAX,
            used_duration_ns: 0,
        };

        assert!(!quota.is_exhausted());
        assert_eq!(quota.remaining_invocations(), 5);

        quota.used_invocations = 10;
        assert!(quota.is_exhausted());
    }

    #[test]
    fn test_glob_matching() {
        // Recursive glob
        assert!(matches_glob("/workspace/**", "/workspace/src/main.rs"));
        assert!(matches_glob("/workspace/**", "/workspace/a/b/c/d"));
        assert!(!matches_glob("/workspace/**", "/home/user/file"));

        // Single level glob
        assert!(matches_glob("/workspace/*", "/workspace/file.txt"));
        assert!(!matches_glob("/workspace/*", "/workspace/dir/file.txt"));

        // Exact match
        assert!(matches_glob("/workspace/.env", "/workspace/.env"));
        assert!(!matches_glob("/workspace/.env", "/workspace/.env.local"));
    }
}
