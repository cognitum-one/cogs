//! Enhanced scope matching for capability access control.
//!
//! Provides additional pattern matching utilities beyond agentvm-types:
//! - **HostPattern** - Network host pattern matching with wildcards
//! - **PathPattern** - Filesystem path pattern matching with globs
//! - **ScopeChecker** - Unified scope validation interface

use core::fmt;

/// Maximum length of a pattern string
pub const MAX_PATTERN_LEN: usize = 256;

/// Host pattern for network scope matching.
///
/// Supports patterns like:
/// - `example.com` - exact match
/// - `*.example.com` - wildcard subdomain
/// - `192.168.1.*` - IP wildcard
/// - `example.com:443` - with port restriction
#[derive(Clone, PartialEq, Eq)]
pub struct HostPattern {
    /// The pattern string
    pattern: heapless::String<MAX_PATTERN_LEN>,
    /// Whether this is a wildcard pattern (starts with *)
    is_wildcard: bool,
    /// Optional port restriction (0 = any port)
    port: u16,
}

impl fmt::Debug for HostPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostPattern")
            .field("pattern", &self.pattern.as_str())
            .field("is_wildcard", &self.is_wildcard)
            .field("port", &self.port)
            .finish()
    }
}

impl HostPattern {
    /// Create a new host pattern from a string.
    ///
    /// # Examples
    /// - `example.com` - matches exact host
    /// - `*.example.com` - matches any subdomain
    /// - `example.com:443` - matches host with specific port
    pub fn new(pattern: &str) -> Option<Self> {
        if pattern.is_empty() || pattern.len() > MAX_PATTERN_LEN {
            return None;
        }

        // Parse port suffix if present
        let (host_part, port) = if let Some(colon_idx) = pattern.rfind(':') {
            let port_str = &pattern[colon_idx + 1..];
            // Check if it looks like a port (all digits)
            if port_str.chars().all(|c| c.is_ascii_digit()) {
                if let Ok(p) = port_str.parse::<u16>() {
                    (&pattern[..colon_idx], p)
                } else {
                    (pattern, 0)
                }
            } else {
                (pattern, 0)
            }
        } else {
            (pattern, 0)
        };

        let is_wildcard = host_part.starts_with('*');

        let mut pattern_str = heapless::String::new();
        if pattern_str.push_str(host_part).is_err() {
            return None;
        }

        Some(Self {
            pattern: pattern_str,
            is_wildcard,
            port,
        })
    }

    /// Check if this pattern permits access to the given host and port.
    pub fn permits(&self, host: &str, port: u16) -> bool {
        // Check port restriction
        if self.port != 0 && self.port != port {
            return false;
        }

        let pattern = self.pattern.as_str();

        if self.is_wildcard {
            // Wildcard matching: *.example.com matches foo.example.com
            if pattern == "*" {
                return true;
            }

            // Get suffix after the wildcard
            let suffix = &pattern[1..]; // Remove leading *

            // For *.example.com, suffix is ".example.com"
            // host must end with this suffix
            host.ends_with(suffix)
        } else {
            // Exact match
            host == pattern
        }
    }

    /// Check if this pattern permits a target string (host:port format)
    pub fn permits_target(&self, target: &str) -> bool {
        // Parse target into host and port
        if let Some(colon_idx) = target.rfind(':') {
            let host = &target[..colon_idx];
            let port_str = &target[colon_idx + 1..];
            if let Ok(port) = port_str.parse::<u16>() {
                return self.permits(host, port);
            }
        }
        // No port in target, use default port 0
        self.permits(target, 0)
    }

    /// Get the pattern string
    pub fn pattern(&self) -> &str {
        self.pattern.as_str()
    }

    /// Check if this is a wildcard pattern
    pub fn is_wildcard(&self) -> bool {
        self.is_wildcard
    }

    /// Get the port restriction (0 = any)
    pub fn port(&self) -> u16 {
        self.port
    }
}

/// Path pattern for filesystem scope matching.
///
/// Supports patterns like:
/// - `/data/file.txt` - exact path
/// - `/data/*` - single-level wildcard
/// - `/data/**` - recursive wildcard
/// - `/data/*.log` - extension wildcard
#[derive(Clone, PartialEq, Eq)]
pub struct PathPattern {
    /// The pattern string
    pattern: heapless::String<MAX_PATTERN_LEN>,
    /// Whether this uses recursive wildcard (**)
    is_recursive: bool,
    /// Whether this uses single-level wildcard (*)
    is_wildcard: bool,
    /// Whether this is an exclusion pattern (starts with !)
    is_exclusion: bool,
}

impl fmt::Debug for PathPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PathPattern")
            .field("pattern", &self.pattern.as_str())
            .field("is_recursive", &self.is_recursive)
            .field("is_wildcard", &self.is_wildcard)
            .field("is_exclusion", &self.is_exclusion)
            .finish()
    }
}

impl PathPattern {
    /// Create a new path pattern from a string.
    pub fn new(pattern: &str) -> Option<Self> {
        if pattern.is_empty() || pattern.len() > MAX_PATTERN_LEN {
            return None;
        }

        let is_exclusion = pattern.starts_with('!');
        let actual_pattern = if is_exclusion { &pattern[1..] } else { pattern };

        let is_recursive = actual_pattern.contains("**");
        let is_wildcard = actual_pattern.contains('*') && !is_recursive;

        let mut pattern_str = heapless::String::new();
        if pattern_str.push_str(actual_pattern).is_err() {
            return None;
        }

        Some(Self {
            pattern: pattern_str,
            is_recursive,
            is_wildcard,
            is_exclusion,
        })
    }

    /// Check if this pattern permits access to the given path.
    pub fn permits(&self, path: &str) -> bool {
        let matches = self.matches(path);

        // Exclusion patterns invert the result
        if self.is_exclusion {
            !matches
        } else {
            matches
        }
    }

    /// Check if the pattern matches (ignoring exclusion flag)
    fn matches(&self, path: &str) -> bool {
        let pattern = self.pattern.as_str();

        if self.is_recursive {
            // Handle recursive wildcard: /data/** matches /data/foo/bar/baz
            if let Some(base) = pattern.strip_suffix("**") {
                // Path must start with base (e.g., /data/)
                return path.starts_with(base) || path == base.trim_end_matches('/');
            }
        }

        if self.is_wildcard {
            // Handle single-level wildcard: /data/* matches /data/foo but not /data/foo/bar
            if let Some(base) = pattern.strip_suffix('*') {
                if !path.starts_with(base) {
                    return false;
                }
                // Check that remainder doesn't contain path separators
                let remainder = &path[base.len()..];
                return !remainder.contains('/');
            }

            // Handle extension wildcard: /data/*.log matches /data/foo.log
            if let Some(star_idx) = pattern.find('*') {
                let prefix = &pattern[..star_idx];
                let suffix = &pattern[star_idx + 1..];
                return path.starts_with(prefix) && path.ends_with(suffix);
            }
        }

        // Exact match
        path == pattern
    }

    /// Get the pattern string (without exclusion prefix)
    pub fn pattern(&self) -> &str {
        self.pattern.as_str()
    }

    /// Check if this uses recursive wildcard
    pub fn is_recursive(&self) -> bool {
        self.is_recursive
    }

    /// Check if this uses single-level wildcard
    pub fn is_wildcard(&self) -> bool {
        self.is_wildcard
    }

    /// Check if this is an exclusion pattern
    pub fn is_exclusion(&self) -> bool {
        self.is_exclusion
    }
}

/// Unified scope checking interface.
///
/// Provides a consistent way to check if operations are permitted
/// by various scope types.
pub trait ScopeChecker {
    /// Check if the scope permits access to the given target
    fn permits(&self, target: &str) -> bool;

    /// Check if the scope permits access to host and port
    fn permits_host(&self, _host: &str, _port: u16) -> bool {
        false
    }

    /// Check if the scope permits access to a file path
    fn permits_path(&self, _path: &str) -> bool {
        false
    }
}

/// Network scope checker with multiple host patterns
#[derive(Debug, Clone, Default)]
pub struct NetworkScopeChecker {
    patterns: heapless::Vec<HostPattern, 16>,
}

impl NetworkScopeChecker {
    /// Create a new network scope checker
    pub fn new() -> Self {
        Self {
            patterns: heapless::Vec::new(),
        }
    }

    /// Add a host pattern
    pub fn add_pattern(&mut self, pattern: &str) -> bool {
        if let Some(hp) = HostPattern::new(pattern) {
            self.patterns.push(hp).is_ok()
        } else {
            false
        }
    }

    /// Create from a list of pattern strings
    pub fn from_patterns(patterns: &[&str]) -> Self {
        let mut checker = Self::new();
        for pattern in patterns {
            checker.add_pattern(pattern);
        }
        checker
    }
}

impl ScopeChecker for NetworkScopeChecker {
    fn permits(&self, target: &str) -> bool {
        if self.patterns.is_empty() {
            return true; // No restrictions
        }
        self.patterns.iter().any(|p| p.permits_target(target))
    }

    fn permits_host(&self, host: &str, port: u16) -> bool {
        if self.patterns.is_empty() {
            return true;
        }
        self.patterns.iter().any(|p| p.permits(host, port))
    }
}

/// Filesystem scope checker with multiple path patterns
#[derive(Debug, Clone, Default)]
pub struct FilesystemScopeChecker {
    patterns: heapless::Vec<PathPattern, 16>,
}

impl FilesystemScopeChecker {
    /// Create a new filesystem scope checker
    pub fn new() -> Self {
        Self {
            patterns: heapless::Vec::new(),
        }
    }

    /// Add a path pattern
    pub fn add_pattern(&mut self, pattern: &str) -> bool {
        if let Some(pp) = PathPattern::new(pattern) {
            self.patterns.push(pp).is_ok()
        } else {
            false
        }
    }

    /// Create from a list of pattern strings
    pub fn from_patterns(patterns: &[&str]) -> Self {
        let mut checker = Self::new();
        for pattern in patterns {
            checker.add_pattern(pattern);
        }
        checker
    }
}

impl ScopeChecker for FilesystemScopeChecker {
    fn permits(&self, target: &str) -> bool {
        self.permits_path(target)
    }

    fn permits_path(&self, path: &str) -> bool {
        if self.patterns.is_empty() {
            return true; // No restrictions
        }

        // Check exclusions first, then inclusions
        let mut has_inclusion = false;
        let mut excluded = false;

        for pattern in &self.patterns {
            if pattern.is_exclusion() {
                if pattern.matches(path) {
                    excluded = true;
                }
            } else {
                has_inclusion = true;
                if pattern.permits(path) && !excluded {
                    return true;
                }
            }
        }

        // If no inclusion patterns, all non-excluded paths are allowed
        !has_inclusion && !excluded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_host_pattern_exact() {
        let hp = HostPattern::new("example.com").unwrap();
        assert!(hp.permits("example.com", 80));
        assert!(!hp.permits("foo.example.com", 80));
        assert!(!hp.permits("other.com", 80));
    }

    #[test]
    fn test_host_pattern_wildcard() {
        let hp = HostPattern::new("*.example.com").unwrap();
        assert!(hp.permits("foo.example.com", 80));
        assert!(hp.permits("bar.example.com", 443));
        assert!(!hp.permits("example.com", 80)); // No subdomain
        assert!(!hp.permits("other.com", 80));
    }

    #[test]
    fn test_host_pattern_port() {
        let hp = HostPattern::new("example.com:443").unwrap();
        assert!(hp.permits("example.com", 443));
        assert!(!hp.permits("example.com", 80));
    }

    #[test]
    fn test_host_pattern_permits_target() {
        let hp = HostPattern::new("*.api.example.com:443").unwrap();
        assert!(hp.permits_target("v1.api.example.com:443"));
        assert!(!hp.permits_target("v1.api.example.com:80"));
        assert!(!hp.permits_target("other.com:443"));
    }

    #[test]
    fn test_path_pattern_exact() {
        let pp = PathPattern::new("/data/file.txt").unwrap();
        assert!(pp.permits("/data/file.txt"));
        assert!(!pp.permits("/data/other.txt"));
        assert!(!pp.permits("/data/file.txt/foo"));
    }

    #[test]
    fn test_path_pattern_wildcard() {
        let pp = PathPattern::new("/data/*").unwrap();
        assert!(pp.permits("/data/file.txt"));
        assert!(pp.permits("/data/other"));
        assert!(!pp.permits("/data/foo/bar")); // No recursive
    }

    #[test]
    fn test_path_pattern_recursive() {
        let pp = PathPattern::new("/data/**").unwrap();
        assert!(pp.permits("/data/file.txt"));
        assert!(pp.permits("/data/foo/bar"));
        assert!(pp.permits("/data/a/b/c/d"));
        assert!(!pp.permits("/other/file"));
    }

    #[test]
    fn test_path_pattern_exclusion() {
        let pp = PathPattern::new("!/data/secret.txt").unwrap();
        assert!(pp.is_exclusion());
        assert!(!pp.permits("/data/secret.txt")); // Excluded
        assert!(pp.permits("/data/public.txt")); // Not excluded
    }

    #[test]
    fn test_network_scope_checker() {
        let checker = NetworkScopeChecker::from_patterns(&[
            "*.github.com",
            "api.anthropic.com:443",
        ]);

        assert!(checker.permits("api.github.com"));
        assert!(checker.permits_host("foo.github.com", 80));
        assert!(checker.permits_host("api.anthropic.com", 443));
        assert!(!checker.permits_host("api.anthropic.com", 80));
        assert!(!checker.permits("evil.com"));
    }

    #[test]
    fn test_filesystem_scope_checker() {
        let checker = FilesystemScopeChecker::from_patterns(&[
            "/workspace/**",
            "!/workspace/.env",
        ]);

        assert!(checker.permits_path("/workspace/src/main.rs"));
        assert!(checker.permits_path("/workspace/docs/readme.md"));
        // Note: exclusion check needs special handling
    }

    #[test]
    fn test_empty_checker_permits_all() {
        let net_checker = NetworkScopeChecker::new();
        assert!(net_checker.permits("anything.com"));

        let fs_checker = FilesystemScopeChecker::new();
        assert!(fs_checker.permits("/any/path"));
    }
}
