//! Secrets executor for secret capabilities.
//!
//! Handles secret retrieval with multiple provider backends.
//! Secrets are never stored - they are injected per-call.

use crate::config::SecretsConfig;
use crate::error::ExecutorError;
use crate::executor::{Executor, ExecutorResult};
use crate::types::{Capability, CapabilityType, InvokeRequest, Operation, OperationResult, QuotaConsumed};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Trait for secrets providers
#[async_trait]
pub trait SecretsProvider: Send + Sync {
    /// Get a secret value by name
    async fn get_secret(&self, name: &str) -> Result<Option<String>, ExecutorError>;

    /// Check if a secret exists
    async fn has_secret(&self, name: &str) -> bool;

    /// List available secret names (for auditing only)
    async fn list_names(&self) -> Vec<String>;

    /// Get the provider name
    fn name(&self) -> &'static str;
}

/// Secrets provider that reads from environment variables
pub struct EnvSecretsProvider {
    /// Prefix for secret environment variables
    prefix: String,
    /// Allowed secret names (empty = all)
    allowed_names: Vec<String>,
    /// Cached secret names (for listing)
    cached_names: Arc<RwLock<Option<Vec<String>>>>,
}

impl EnvSecretsProvider {
    /// Create a new environment secrets provider
    pub fn new(config: &SecretsConfig) -> Self {
        Self {
            prefix: config.env_prefix.clone(),
            allowed_names: config.allowed_names.clone(),
            cached_names: Arc::new(RwLock::new(None)),
        }
    }

    /// Create with default prefix
    pub fn default_new() -> Self {
        Self {
            prefix: "AGENTVM_SECRET_".to_string(),
            allowed_names: Vec::new(),
            cached_names: Arc::new(RwLock::new(None)),
        }
    }

    /// Check if a secret name is allowed
    fn is_allowed(&self, name: &str) -> bool {
        if self.allowed_names.is_empty() {
            return true;
        }
        self.allowed_names.contains(&name.to_string())
    }

    /// Get the environment variable name for a secret
    fn env_var_name(&self, secret_name: &str) -> String {
        format!("{}{}", self.prefix, secret_name.to_uppercase().replace('-', "_"))
    }
}

#[async_trait]
impl SecretsProvider for EnvSecretsProvider {
    async fn get_secret(&self, name: &str) -> Result<Option<String>, ExecutorError> {
        if !self.is_allowed(name) {
            return Err(ExecutorError::PermissionDenied(format!(
                "Secret '{}' is not in the allowed list",
                name
            )));
        }

        let env_name = self.env_var_name(name);
        debug!("Looking up secret '{}' from env var '{}'", name, env_name);

        Ok(std::env::var(&env_name).ok())
    }

    async fn has_secret(&self, name: &str) -> bool {
        if !self.is_allowed(name) {
            return false;
        }
        let env_name = self.env_var_name(name);
        std::env::var(&env_name).is_ok()
    }

    async fn list_names(&self) -> Vec<String> {
        // Cache the list of names
        let mut cache = self.cached_names.write().await;
        if let Some(ref names) = *cache {
            return names.clone();
        }

        let names: Vec<String> = std::env::vars()
            .filter_map(|(key, _)| {
                if key.starts_with(&self.prefix) {
                    let name = key
                        .trim_start_matches(&self.prefix)
                        .to_lowercase()
                        .replace('_', "-");
                    if self.is_allowed(&name) {
                        Some(name)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        *cache = Some(names.clone());
        names
    }

    fn name(&self) -> &'static str {
        "environment"
    }
}

/// Secrets provider that reads from a file
pub struct FileSecretsProvider {
    /// Path to secrets file
    path: std::path::PathBuf,
    /// Allowed secret names
    allowed_names: Vec<String>,
    /// Cached secrets
    cache: Arc<RwLock<Option<HashMap<String, String>>>>,
}

impl FileSecretsProvider {
    /// Create a new file secrets provider
    pub fn new(path: std::path::PathBuf, allowed_names: Vec<String>) -> Self {
        Self {
            path,
            allowed_names,
            cache: Arc::new(RwLock::new(None)),
        }
    }

    /// Load secrets from file
    async fn load_secrets(&self) -> Result<HashMap<String, String>, ExecutorError> {
        let content = tokio::fs::read_to_string(&self.path)
            .await
            .map_err(|e| ExecutorError::NotFound(format!("Secrets file not found: {}", e)))?;

        // Parse as JSON
        let secrets: HashMap<String, String> = serde_json::from_str(&content)
            .map_err(|e| ExecutorError::Internal(format!("Invalid secrets file format: {}", e)))?;

        Ok(secrets)
    }

    /// Check if a secret name is allowed
    fn is_allowed(&self, name: &str) -> bool {
        if self.allowed_names.is_empty() {
            return true;
        }
        self.allowed_names.contains(&name.to_string())
    }
}

#[async_trait]
impl SecretsProvider for FileSecretsProvider {
    async fn get_secret(&self, name: &str) -> Result<Option<String>, ExecutorError> {
        if !self.is_allowed(name) {
            return Err(ExecutorError::PermissionDenied(format!(
                "Secret '{}' is not in the allowed list",
                name
            )));
        }

        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(ref secrets) = *cache {
                return Ok(secrets.get(name).cloned());
            }
        }

        // Load and cache
        let secrets = self.load_secrets().await?;
        let result = secrets.get(name).cloned();

        let mut cache = self.cache.write().await;
        *cache = Some(secrets);

        Ok(result)
    }

    async fn has_secret(&self, name: &str) -> bool {
        if !self.is_allowed(name) {
            return false;
        }

        match self.get_secret(name).await {
            Ok(Some(_)) => true,
            _ => false,
        }
    }

    async fn list_names(&self) -> Vec<String> {
        match self.load_secrets().await {
            Ok(secrets) => secrets
                .keys()
                .filter(|k| self.is_allowed(k))
                .cloned()
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    fn name(&self) -> &'static str {
        "file"
    }
}

/// Null secrets provider that returns no secrets
pub struct NullSecretsProvider;

#[async_trait]
impl SecretsProvider for NullSecretsProvider {
    async fn get_secret(&self, _name: &str) -> Result<Option<String>, ExecutorError> {
        Ok(None)
    }

    async fn has_secret(&self, _name: &str) -> bool {
        false
    }

    async fn list_names(&self) -> Vec<String> {
        Vec::new()
    }

    fn name(&self) -> &'static str {
        "null"
    }
}

/// Secrets executor that wraps a provider
pub struct SecretsExecutor {
    /// The secrets provider
    provider: Arc<dyn SecretsProvider>,
}

impl SecretsExecutor {
    /// Create a new secrets executor
    pub fn new(provider: Arc<dyn SecretsProvider>) -> Self {
        Self { provider }
    }

    /// Create from environment provider
    pub fn from_env(config: &SecretsConfig) -> Self {
        Self {
            provider: Arc::new(EnvSecretsProvider::new(config)),
        }
    }
}

#[async_trait]
impl Executor for SecretsExecutor {
    async fn execute(
        &self,
        capability: &Capability,
        request: &InvokeRequest,
    ) -> Result<ExecutorResult, ExecutorError> {
        // Verify capability type
        if capability.cap_type != CapabilityType::SecretRead {
            return Err(ExecutorError::PermissionDenied(
                "Capability does not allow secret reads".to_string(),
            ));
        }

        // Verify capability allows this operation
        if !capability.scope.permits(&request.operation) {
            return Err(ExecutorError::PermissionDenied(
                "Capability scope does not permit this secret".to_string(),
            ));
        }

        match &request.operation {
            Operation::SecretRead { name } => {
                let start = Instant::now();

                let value = self.provider.get_secret(name).await?;
                let value = value.ok_or_else(|| {
                    ExecutorError::NotFound(format!("Secret '{}' not found", name))
                })?;

                let elapsed = start.elapsed();
                let bytes = value.len();

                info!(
                    "Retrieved secret '{}' ({} bytes) in {:?}",
                    name, bytes, elapsed
                );

                Ok(ExecutorResult::new(
                    OperationResult::SecretValue { value },
                    QuotaConsumed::single(bytes as u64, elapsed.as_nanos() as u64),
                ))
            }
            _ => Err(ExecutorError::NotSupported(format!(
                "Secrets executor cannot handle operation: {:?}",
                std::mem::discriminant(&request.operation)
            ))),
        }
    }

    fn can_handle(&self, capability: &Capability) -> bool {
        capability.cap_type.is_secret()
    }

    fn name(&self) -> &'static str {
        "secrets"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[tokio::test]
    async fn test_env_secrets_provider() {
        // Set a test secret
        std::env::set_var("AGENTVM_SECRET_TEST_KEY", "test_value");

        let config = SecretsConfig {
            env_prefix: "AGENTVM_SECRET_".to_string(),
            ..Default::default()
        };
        let provider = EnvSecretsProvider::new(&config);

        // Should find the secret
        let value = provider.get_secret("test_key").await.unwrap();
        assert_eq!(value, Some("test_value".to_string()));

        // Should not find non-existent secret
        let missing = provider.get_secret("missing").await.unwrap();
        assert_eq!(missing, None);

        // Cleanup
        std::env::remove_var("AGENTVM_SECRET_TEST_KEY");
    }

    #[tokio::test]
    async fn test_env_secrets_allowed_list() {
        std::env::set_var("AGENTVM_SECRET_ALLOWED", "allowed_value");
        std::env::set_var("AGENTVM_SECRET_BLOCKED", "blocked_value");

        let config = SecretsConfig {
            env_prefix: "AGENTVM_SECRET_".to_string(),
            allowed_names: vec!["allowed".to_string()],
            ..Default::default()
        };
        let provider = EnvSecretsProvider::new(&config);

        // Should find allowed secret
        let allowed = provider.get_secret("allowed").await.unwrap();
        assert_eq!(allowed, Some("allowed_value".to_string()));

        // Should deny blocked secret
        let blocked = provider.get_secret("blocked").await;
        assert!(blocked.is_err());

        // Cleanup
        std::env::remove_var("AGENTVM_SECRET_ALLOWED");
        std::env::remove_var("AGENTVM_SECRET_BLOCKED");
    }

    #[tokio::test]
    async fn test_file_secrets_provider() {
        // Create a temp secrets file
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"api_key": "secret123", "database_url": "postgres://localhost"}}"#
        )
        .unwrap();

        let provider = FileSecretsProvider::new(
            file.path().to_path_buf(),
            Vec::new(),
        );

        // Should find secrets
        let api_key = provider.get_secret("api_key").await.unwrap();
        assert_eq!(api_key, Some("secret123".to_string()));

        let db_url = provider.get_secret("database_url").await.unwrap();
        assert_eq!(db_url, Some("postgres://localhost".to_string()));

        // Should not find non-existent
        let missing = provider.get_secret("missing").await.unwrap();
        assert_eq!(missing, None);
    }

    #[tokio::test]
    async fn test_null_secrets_provider() {
        let provider = NullSecretsProvider;

        let value = provider.get_secret("anything").await.unwrap();
        assert_eq!(value, None);

        assert!(!provider.has_secret("anything").await);
        assert!(provider.list_names().await.is_empty());
    }
}
