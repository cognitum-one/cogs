//! Core types for the capability proxy.
//!
//! These types mirror the definitions from ADR-005 and provide the foundation
//! for capability-based security in Agentic VM.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use uuid::Uuid;

/// Unique identifier for a capability instance (128-bit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilityId(pub u128);

impl CapabilityId {
    /// Generate a new random capability ID
    pub fn generate() -> Self {
        Self(Uuid::new_v4().as_u128())
    }

    /// Create from raw bytes
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(u128::from_le_bytes(bytes))
    }

    /// Convert to raw bytes
    pub fn to_bytes(self) -> [u8; 16] {
        self.0.to_le_bytes()
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cap:{:032x}", self.0)
    }
}

/// Unique identifier for a capsule (128-bit)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapsuleId(pub [u8; 16]);

impl CapsuleId {
    /// Generate a new random capsule ID
    pub fn generate() -> Self {
        Self(Uuid::new_v4().into_bytes())
    }

    /// Create from a string identifier (hashed to 128 bits)
    pub fn from_name(name: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(name.as_bytes());
        let hash = hasher.finalize();
        let mut bytes = [0u8; 16];
        bytes.copy_from_slice(&hash[..16]);
        Self(bytes)
    }
}

impl fmt::Display for CapsuleId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "capsule:{}", hex::encode(&self.0))
    }
}

/// Core capability type enumeration per ADR-005
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum CapabilityType {
    // Network capabilities (0x01xx)
    /// HTTP/HTTPS requests
    NetworkHttp = 0x0100,
    /// Raw TCP connections
    NetworkTcp = 0x0101,
    /// DNS resolution
    NetworkDns = 0x0102,

    // Filesystem capabilities (0x02xx)
    /// Read files
    FileRead = 0x0200,
    /// Write files
    FileWrite = 0x0201,
    /// Delete files
    FileDelete = 0x0202,
    /// List directory contents
    DirectoryList = 0x0203,

    // Process capabilities (0x03xx)
    /// Spawn subprocesses
    ProcessSpawn = 0x0300,
    /// Send signals to processes
    ProcessSignal = 0x0301,

    // Secret capabilities (0x04xx)
    /// Read secrets
    SecretRead = 0x0400,
    /// Rotate secrets
    SecretRotate = 0x0401,

    // Clock capabilities (0x05xx)
    /// Read wall clock
    ClockRead = 0x0500,
    /// Read monotonic clock
    ClockMonotonic = 0x0501,

    // Random capabilities (0x06xx)
    /// Cryptographic random
    RandomSecure = 0x0600,
    /// Fast pseudo-random
    RandomInsecure = 0x0601,

    // Evidence capabilities (0x07xx)
    /// Append to evidence log
    EvidenceAppend = 0x0700,
    /// Read own evidence
    EvidenceRead = 0x0701,
}

impl CapabilityType {
    /// Get the type code as u16
    pub fn code(&self) -> u16 {
        *self as u16
    }

    /// Create from type code
    pub fn from_code(code: u16) -> Option<Self> {
        match code {
            0x0100 => Some(Self::NetworkHttp),
            0x0101 => Some(Self::NetworkTcp),
            0x0102 => Some(Self::NetworkDns),
            0x0200 => Some(Self::FileRead),
            0x0201 => Some(Self::FileWrite),
            0x0202 => Some(Self::FileDelete),
            0x0203 => Some(Self::DirectoryList),
            0x0300 => Some(Self::ProcessSpawn),
            0x0301 => Some(Self::ProcessSignal),
            0x0400 => Some(Self::SecretRead),
            0x0401 => Some(Self::SecretRotate),
            0x0500 => Some(Self::ClockRead),
            0x0501 => Some(Self::ClockMonotonic),
            0x0600 => Some(Self::RandomSecure),
            0x0601 => Some(Self::RandomInsecure),
            0x0700 => Some(Self::EvidenceAppend),
            0x0701 => Some(Self::EvidenceRead),
            _ => None,
        }
    }

    /// Check if this is a network capability
    pub fn is_network(&self) -> bool {
        matches!(
            self,
            Self::NetworkHttp | Self::NetworkTcp | Self::NetworkDns
        )
    }

    /// Check if this is a filesystem capability
    pub fn is_filesystem(&self) -> bool {
        matches!(
            self,
            Self::FileRead | Self::FileWrite | Self::FileDelete | Self::DirectoryList
        )
    }

    /// Check if this is a secret capability
    pub fn is_secret(&self) -> bool {
        matches!(self, Self::SecretRead | Self::SecretRotate)
    }
}

/// Rights bit vector (32 bits)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Rights(pub u32);

impl Rights {
    /// Read right
    pub const READ: u32 = 1 << 0;
    /// Write right
    pub const WRITE: u32 = 1 << 1;
    /// Execute right
    pub const EXECUTE: u32 = 1 << 2;
    /// Delete right
    pub const DELETE: u32 = 1 << 3;
    /// Can create child capabilities
    pub const DELEGATE: u32 = 1 << 4;
    /// Can read audit logs
    pub const AUDIT: u32 = 1 << 5;

    /// Create a new rights value
    pub fn new(bits: u32) -> Self {
        Self(bits)
    }

    /// Check if a right is present
    pub fn has(&self, right: u32) -> bool {
        self.0 & right == right
    }

    /// Intersect with another rights value
    pub fn intersect(&self, other: &Rights) -> Rights {
        Rights(self.0 & other.0)
    }

    /// Check if this is a subset of another rights value
    pub fn is_subset_of(&self, other: &Rights) -> bool {
        (self.0 & other.0) == self.0
    }

    /// Get all rights
    pub fn all() -> Self {
        Self(Self::READ | Self::WRITE | Self::EXECUTE | Self::DELETE | Self::DELEGATE | Self::AUDIT)
    }
}

/// Quota tracking for capability usage
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Quota {
    /// Maximum invocations allowed
    pub max_invocations: u64,
    /// Invocations used
    pub used_invocations: u64,

    /// Maximum bytes transferred
    pub max_bytes: u64,
    /// Bytes used
    pub used_bytes: u64,

    /// Maximum wall clock time (nanoseconds)
    pub max_duration_ns: u64,
    /// Duration used (nanoseconds)
    pub used_duration_ns: u64,
}

impl Quota {
    /// Create a new quota
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

    /// Check if quota is exhausted
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

    /// Get remaining duration
    pub fn remaining_duration_ns(&self) -> u64 {
        self.max_duration_ns.saturating_sub(self.used_duration_ns)
    }

    /// Deduct usage from quota
    pub fn deduct(&mut self, consumed: &QuotaConsumed) {
        self.used_invocations = self.used_invocations.saturating_add(consumed.invocations);
        self.used_bytes = self.used_bytes.saturating_add(consumed.bytes);
        self.used_duration_ns = self.used_duration_ns.saturating_add(consumed.duration_ns);
    }

    /// Check if deduction would exhaust quota
    pub fn would_exhaust(&self, consumed: &QuotaConsumed) -> bool {
        self.used_invocations + consumed.invocations > self.max_invocations
            || self.used_bytes + consumed.bytes > self.max_bytes
            || self.used_duration_ns + consumed.duration_ns > self.max_duration_ns
    }
}

/// Quota consumed by a single operation
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QuotaConsumed {
    /// Invocations (usually 1)
    pub invocations: u64,
    /// Request + response size in bytes
    pub bytes: u64,
    /// Wall clock time in nanoseconds
    pub duration_ns: u64,
}

impl QuotaConsumed {
    /// Create a new consumed quota record
    pub fn new(invocations: u64, bytes: u64, duration_ns: u64) -> Self {
        Self {
            invocations,
            bytes,
            duration_ns,
        }
    }

    /// Create a single invocation record
    pub fn single(bytes: u64, duration_ns: u64) -> Self {
        Self::new(1, bytes, duration_ns)
    }
}

/// Host pattern for network scope (e.g., "*.github.com")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostPattern(pub String);

impl HostPattern {
    /// Create a new host pattern
    pub fn new(pattern: impl Into<String>) -> Self {
        Self(pattern.into())
    }

    /// Check if a host matches this pattern
    pub fn matches(&self, host: &str) -> bool {
        if self.0 == "*" {
            return true;
        }
        if self.0.starts_with("*.") {
            let suffix = &self.0[1..]; // ".example.com"
            host.ends_with(suffix) || host == &self.0[2..]
        } else {
            host == self.0
        }
    }
}

/// Port range for network scope
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortRange {
    /// Start port (inclusive)
    pub start: u16,
    /// End port (inclusive)
    pub end: u16,
}

impl PortRange {
    /// Create a single port
    pub fn single(port: u16) -> Self {
        Self {
            start: port,
            end: port,
        }
    }

    /// Create a port range
    pub fn range(start: u16, end: u16) -> Self {
        Self { start, end }
    }

    /// Check if a port is in this range
    pub fn contains(&self, port: u16) -> bool {
        port >= self.start && port <= self.end
    }
}

/// Protocol for network scope
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Protocol {
    /// HTTP only
    Http,
    /// HTTPS only
    Https,
    /// Both HTTP and HTTPS
    HttpOrHttps,
    /// Raw TCP
    Tcp,
    /// UDP
    Udp,
}

/// Path pattern for filesystem scope (e.g., "/workspace/**")
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathPattern(pub String);

impl PathPattern {
    /// Create a new path pattern
    pub fn new(pattern: impl Into<String>) -> Self {
        Self(pattern.into())
    }

    /// Check if this is a negation pattern (starts with !)
    pub fn is_negation(&self) -> bool {
        self.0.starts_with('!')
    }

    /// Get the pattern without negation prefix
    pub fn pattern(&self) -> &str {
        if self.is_negation() {
            &self.0[1..]
        } else {
            &self.0
        }
    }

    /// Check if a path matches this pattern (using glob)
    pub fn matches(&self, path: &str) -> bool {
        let pattern = self.pattern();

        // Use glob matching
        if let Ok(glob) = glob::Pattern::new(pattern) {
            glob.matches(path)
        } else {
            // Fallback to simple prefix matching
            path.starts_with(pattern.trim_end_matches("**"))
        }
    }
}

/// File operations bitmap
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct FileOperations(pub u8);

impl FileOperations {
    /// Read operation
    pub const READ: u8 = 1 << 0;
    /// Write operation
    pub const WRITE: u8 = 1 << 1;
    /// Delete operation
    pub const DELETE: u8 = 1 << 2;
    /// List operation
    pub const LIST: u8 = 1 << 3;

    /// Create new file operations
    pub fn new(bits: u8) -> Self {
        Self(bits)
    }

    /// Check if operation is allowed
    pub fn allows(&self, op: u8) -> bool {
        self.0 & op == op
    }

    /// All operations
    pub fn all() -> Self {
        Self(Self::READ | Self::WRITE | Self::DELETE | Self::LIST)
    }
}

/// Scope restricts what a capability can access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CapabilityScope {
    /// Network scope: specific hosts/ports
    Network {
        /// Allowed host patterns
        hosts: Vec<HostPattern>,
        /// Allowed port ranges
        ports: Vec<PortRange>,
        /// Allowed protocols
        protocols: Vec<Protocol>,
    },

    /// Filesystem scope: path patterns
    Filesystem {
        /// Path patterns (glob-style, ! for negation)
        paths: Vec<PathPattern>,
        /// Allowed operations
        operations: FileOperations,
    },

    /// Process scope: allowed executables
    Process {
        /// Allowed executable names
        executables: Vec<String>,
        /// Argument pattern (regex)
        args_pattern: Option<String>,
        /// Allowed environment variables
        env_allowlist: Vec<String>,
    },

    /// Secrets scope: named secrets
    Secrets {
        /// Allowed secret names
        names: Vec<String>,
    },

    /// Clock scope (no restrictions)
    Clock,

    /// Random scope (no restrictions)
    Random,

    /// Evidence scope
    Evidence {
        /// Can read own evidence
        read_own: bool,
        /// Can append to evidence
        append: bool,
    },
}

impl CapabilityScope {
    /// Check if an operation is permitted by this scope
    pub fn permits(&self, operation: &Operation) -> bool {
        match (self, operation) {
            (
                CapabilityScope::Network {
                    hosts,
                    ports,
                    protocols,
                },
                Operation::HttpRequest { url, .. },
            ) => {
                // Parse URL to check host and port
                if let Ok(parsed) = url::Url::parse(url) {
                    let host = parsed.host_str().unwrap_or("");
                    let port = parsed.port().unwrap_or(match parsed.scheme() {
                        "https" => 443,
                        "http" => 80,
                        _ => 0,
                    });
                    let protocol = match parsed.scheme() {
                        "https" => Protocol::Https,
                        "http" => Protocol::Http,
                        _ => return false,
                    };

                    let host_ok = hosts.is_empty() || hosts.iter().any(|h| h.matches(host));
                    let port_ok = ports.is_empty() || ports.iter().any(|p| p.contains(port));
                    let protocol_ok = protocols.is_empty()
                        || protocols.iter().any(|p| {
                            *p == protocol || *p == Protocol::HttpOrHttps
                        });

                    host_ok && port_ok && protocol_ok
                } else {
                    false
                }
            }
            (
                CapabilityScope::Filesystem { paths, operations },
                Operation::FileRead { path, .. },
            ) => {
                operations.allows(FileOperations::READ) && Self::path_allowed(paths, path)
            }
            (
                CapabilityScope::Filesystem { paths, operations },
                Operation::FileWrite { path, .. },
            ) => {
                operations.allows(FileOperations::WRITE) && Self::path_allowed(paths, path)
            }
            (
                CapabilityScope::Filesystem { paths, operations },
                Operation::FileDelete { path },
            ) => {
                operations.allows(FileOperations::DELETE) && Self::path_allowed(paths, path)
            }
            (
                CapabilityScope::Filesystem { paths, operations },
                Operation::DirectoryList { path },
            ) => {
                operations.allows(FileOperations::LIST) && Self::path_allowed(paths, path)
            }
            (CapabilityScope::Secrets { names }, Operation::SecretRead { name }) => {
                names.is_empty() || names.contains(name)
            }
            _ => false,
        }
    }

    /// Check if a path is allowed by path patterns
    fn path_allowed(patterns: &[PathPattern], path: &str) -> bool {
        if patterns.is_empty() {
            return false;
        }

        let mut allowed = false;

        for pattern in patterns {
            if pattern.is_negation() {
                if pattern.matches(path) {
                    allowed = false;
                }
            } else if pattern.matches(path) {
                allowed = true;
            }
        }

        allowed
    }

    /// Intersect with another scope (for derivation)
    pub fn intersect(&self, other: &CapabilityScope) -> Option<CapabilityScope> {
        match (self, other) {
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
                // Take more restrictive (intersection)
                let hosts = if h2.is_empty() {
                    h1.clone()
                } else {
                    h2.clone()
                };
                let ports = if p2.is_empty() {
                    p1.clone()
                } else {
                    p2.clone()
                };
                let protocols = if pr2.is_empty() {
                    pr1.clone()
                } else {
                    pr2.clone()
                };

                Some(CapabilityScope::Network {
                    hosts,
                    ports,
                    protocols,
                })
            }
            (
                CapabilityScope::Filesystem {
                    paths: p1,
                    operations: o1,
                },
                CapabilityScope::Filesystem {
                    paths: p2,
                    operations: o2,
                },
            ) => {
                // Combine paths, intersect operations
                let mut paths = p1.clone();
                paths.extend(p2.iter().cloned());
                let operations = FileOperations(o1.0 & o2.0);

                Some(CapabilityScope::Filesystem { paths, operations })
            }
            (CapabilityScope::Secrets { names: n1 }, CapabilityScope::Secrets { names: n2 }) => {
                // Intersect secret names
                let names: Vec<_> = if n2.is_empty() {
                    n1.clone()
                } else {
                    n1.iter().filter(|n| n2.contains(n)).cloned().collect()
                };

                Some(CapabilityScope::Secrets { names })
            }
            _ => None,
        }
    }
}

/// Cryptographic proof of capability issuance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityProof {
    /// Key ID used for signing
    pub key_id: String,
    /// Signature bytes
    pub signature: Vec<u8>,
    /// Timestamp of signing
    pub timestamp_ns: u64,
}

impl CapabilityProof {
    /// Create a placeholder proof (for testing)
    pub fn placeholder() -> Self {
        Self {
            key_id: "test".to_string(),
            signature: vec![0u8; 64],
            timestamp_ns: 0,
        }
    }

    /// Verify the proof (stub - would use real crypto)
    pub fn verify(&self, _cap: &Capability) -> bool {
        // TODO: Implement real signature verification
        !self.signature.is_empty()
    }
}

/// A capability token granting specific rights
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    /// Unique identifier for this capability instance
    pub id: CapabilityId,

    /// Capsule that owns this capability
    pub capsule_id: CapsuleId,

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

    /// Whether the capability has been revoked
    #[serde(default)]
    pub revoked: bool,
}

impl Capability {
    /// Check if the capability is currently valid
    pub fn is_valid(&self, current_time_ns: u64) -> bool {
        !self.revoked && !self.is_expired(current_time_ns) && !self.quota.is_exhausted()
    }

    /// Check if the capability has expired
    pub fn is_expired(&self, current_time_ns: u64) -> bool {
        current_time_ns >= self.expires_at
    }

    /// Check if the capability has been revoked
    pub fn is_revoked(&self) -> bool {
        self.revoked
    }
}

/// Grant request for creating a new capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityGrant {
    /// Type of capability to grant
    pub cap_type: CapabilityType,
    /// Scope for the capability
    pub scope: CapabilityScope,
    /// Rights to grant
    pub rights: Rights,
    /// Quota limits
    pub quota: Quota,
    /// Duration in nanoseconds until expiry
    pub duration_ns: u64,
}

impl Capability {
    /// Create a capability from a grant
    pub fn from_grant(
        capsule_id: CapsuleId,
        grant: CapabilityGrant,
        current_time_ns: u64,
    ) -> Self {
        Self {
            id: CapabilityId::generate(),
            capsule_id,
            cap_type: grant.cap_type,
            scope: grant.scope,
            rights: grant.rights,
            quota: grant.quota,
            expires_at: current_time_ns.saturating_add(grant.duration_ns),
            parent: None,
            proof: CapabilityProof::placeholder(),
            revoked: false,
        }
    }
}

/// Operation to perform (type-specific)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    // Network operations
    /// HTTP request
    HttpRequest {
        /// HTTP method
        method: String,
        /// Full URL
        url: String,
        /// Headers as key-value pairs
        headers: Vec<(String, String)>,
        /// Optional request body
        body: Option<Vec<u8>>,
    },
    /// TCP connection
    TcpConnect {
        /// Target host
        host: String,
        /// Target port
        port: u16,
    },
    /// DNS resolution
    DnsResolve {
        /// Name to resolve
        name: String,
    },

    // Filesystem operations
    /// Read file
    FileRead {
        /// File path
        path: String,
        /// Byte offset
        offset: u64,
        /// Number of bytes to read
        len: u64,
    },
    /// Write file
    FileWrite {
        /// File path
        path: String,
        /// Byte offset
        offset: u64,
        /// Data to write
        data: Vec<u8>,
    },
    /// Delete file
    FileDelete {
        /// File path
        path: String,
    },
    /// List directory
    DirectoryList {
        /// Directory path
        path: String,
    },

    // Process operations
    /// Spawn process
    ProcessSpawn {
        /// Executable path
        executable: String,
        /// Arguments
        args: Vec<String>,
        /// Environment variables
        env: Vec<(String, String)>,
    },
    /// Send signal
    ProcessSignal {
        /// Process ID
        pid: u32,
        /// Signal number
        signal: i32,
    },

    // Secret operations
    /// Read secret
    SecretRead {
        /// Secret name
        name: String,
    },
}

/// Result of an operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OperationResult {
    /// HTTP response
    HttpResponse {
        /// Status code
        status: u16,
        /// Response headers
        headers: Vec<(String, String)>,
        /// Response body
        body: Vec<u8>,
    },
    /// TCP connection established
    TcpConnected {
        /// Local address
        local_addr: String,
    },
    /// DNS resolution result
    DnsResolved {
        /// Resolved addresses
        addresses: Vec<String>,
    },
    /// File read result
    FileData {
        /// Data read
        data: Vec<u8>,
    },
    /// File write result
    FileWritten {
        /// Bytes written
        bytes_written: u64,
    },
    /// File deleted
    FileDeleted,
    /// Directory listing
    DirectoryEntries {
        /// Entry names
        entries: Vec<String>,
    },
    /// Process spawned
    ProcessSpawned {
        /// Process ID
        pid: u32,
    },
    /// Signal sent
    SignalSent,
    /// Secret value
    SecretValue {
        /// Secret data
        value: String,
    },
    /// Operation failed
    Error {
        /// Error code
        code: String,
        /// Error message
        message: String,
    },
}

/// Invoke request from guest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeRequest {
    /// The operation to perform
    pub operation: Operation,
    /// Operation-specific parameters (additional context)
    pub params: serde_json::Value,
    /// Deadline for this invocation (monotonic ns)
    pub deadline_ns: u64,
    /// Idempotency key for retry safety
    pub idempotency_key: Option<[u8; 16]>,
}

/// Invoke response to guest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeResponse {
    /// Operation result
    pub result: OperationResult,
    /// Quota consumed by this invocation
    pub quota_consumed: QuotaConsumed,
    /// Evidence hash for this invocation
    pub evidence_hash: [u8; 32],
}

/// Derive request for creating child capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeriveRequest {
    /// New scope (must be subset of parent)
    pub scope: CapabilityScope,
    /// New rights (must be subset of parent)
    pub rights: Rights,
    /// New quota (must be subset of parent remaining)
    pub quota: Quota,
    /// New expiry (must be <= parent expiry)
    pub expires_at: Option<u64>,
}

/// Capability validation result
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationResult {
    /// Capability is valid
    Valid,
    /// Capability has expired
    Expired,
    /// Capability has been revoked
    Revoked,
    /// Quota has been exhausted
    QuotaExhausted,
    /// Operation violates scope
    ScopeViolation,
    /// Signature verification failed
    InvalidSignature,
    /// Capability not found
    NotFound,
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
            Self::NotFound => write!(f, "not found"),
        }
    }
}

// Hex encoding helper (minimal implementation to avoid extra dependency)
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_id_generation() {
        let id1 = CapabilityId::generate();
        let id2 = CapabilityId::generate();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_host_pattern_matching() {
        let pattern = HostPattern::new("*.github.com");
        assert!(pattern.matches("api.github.com"));
        assert!(pattern.matches("raw.github.com"));
        assert!(pattern.matches("github.com"));
        assert!(!pattern.matches("githubcom"));
        assert!(!pattern.matches("evil.github.com.attacker.com"));

        let exact = HostPattern::new("api.example.com");
        assert!(exact.matches("api.example.com"));
        assert!(!exact.matches("other.example.com"));

        let wildcard = HostPattern::new("*");
        assert!(wildcard.matches("anything.com"));
    }

    #[test]
    fn test_port_range() {
        let single = PortRange::single(443);
        assert!(single.contains(443));
        assert!(!single.contains(80));

        let range = PortRange::range(8000, 9000);
        assert!(range.contains(8000));
        assert!(range.contains(8500));
        assert!(range.contains(9000));
        assert!(!range.contains(7999));
        assert!(!range.contains(9001));
    }

    #[test]
    fn test_quota_exhaustion() {
        let mut quota = Quota::new(10, 1000, 1_000_000_000);
        assert!(!quota.is_exhausted());

        quota.used_invocations = 10;
        assert!(quota.is_exhausted());

        quota.used_invocations = 5;
        quota.used_bytes = 1000;
        assert!(quota.is_exhausted());
    }

    #[test]
    fn test_rights_operations() {
        let rights = Rights::new(Rights::READ | Rights::WRITE);
        assert!(rights.has(Rights::READ));
        assert!(rights.has(Rights::WRITE));
        assert!(!rights.has(Rights::DELETE));

        let other = Rights::new(Rights::READ | Rights::DELETE);
        let intersected = rights.intersect(&other);
        assert!(intersected.has(Rights::READ));
        assert!(!intersected.has(Rights::WRITE));
        assert!(!intersected.has(Rights::DELETE));
    }
}
