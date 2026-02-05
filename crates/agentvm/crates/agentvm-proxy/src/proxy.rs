//! Main capability proxy implementation
//!
//! The CapabilityProxy is the central component that:
//! - Manages capability grants and revocations
//! - Validates and routes capability invocations
//! - Tracks budget consumption and quota
//! - Logs all operations for evidence chain

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use dashmap::DashMap;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::config::ProxyConfig;
use crate::error::{BudgetError, GrantError, InvokeError, ProxyError, RevokeError};
use crate::evidence::EvidenceLogger;
use crate::executor::{
    filesystem::LocalFilesystemExecutor,
    network::HttpNetworkExecutor,
    secrets::{EnvSecretsProvider, SecretsExecutor, SecretsProvider},
    Executor, ExecutorResult,
};
use crate::types::{
    Capability, CapabilityGrant, CapabilityId, CapabilityType, CapsuleId,
    InvokeRequest, InvokeResponse, OperationResult, Quota, QuotaConsumed, ValidationResult,
};
use crate::vsock::{ConnectionHandler, VsockListener};
use crate::wire::{ErrorPayload, MessageEnvelope, MessageType};

/// Main capability proxy managing capsule access
pub struct CapabilityProxy {
    /// Configuration
    config: ProxyConfig,

    /// Active capabilities indexed by ID
    capabilities: Arc<RwLock<HashMap<CapabilityId, Capability>>>,

    /// Capabilities indexed by capsule ID for fast revocation
    capsule_capabilities: Arc<RwLock<HashMap<CapsuleId, Vec<CapabilityId>>>>,

    /// Network executor
    network_executor: Arc<HttpNetworkExecutor>,

    /// Filesystem executor
    filesystem_executor: Arc<LocalFilesystemExecutor>,

    /// Secrets executor
    secrets_executor: Arc<SecretsExecutor>,

    /// Evidence logger
    evidence_logger: Arc<EvidenceLogger>,

    /// Shutdown signal sender
    shutdown_tx: Option<mpsc::Sender<()>>,
}

impl CapabilityProxy {
    /// Create a new capability proxy
    pub async fn new(config: ProxyConfig) -> Result<Self, ProxyError> {
        // Initialize evidence logger
        let evidence_logger = Arc::new(
            EvidenceLogger::new(&config.evidence)
                .await
                .map_err(|e| ProxyError::Evidence(e.to_string()))?,
        );

        // Initialize executors
        let network_executor = Arc::new(
            HttpNetworkExecutor::new(&config.network)
                .map_err(|e| ProxyError::Network(e.to_string()))?,
        );

        let filesystem_executor = Arc::new(
            LocalFilesystemExecutor::new(&config.filesystem)
                .map_err(|e| ProxyError::Internal(e.to_string()))?,
        );

        let secrets_executor = Arc::new(SecretsExecutor::from_env(&config.secrets));

        info!("Capability proxy initialized");

        Ok(Self {
            config,
            capabilities: Arc::new(RwLock::new(HashMap::new())),
            capsule_capabilities: Arc::new(RwLock::new(HashMap::new())),
            network_executor,
            filesystem_executor,
            secrets_executor,
            evidence_logger,
            shutdown_tx: None,
        })
    }

    /// Get current time in nanoseconds
    fn current_time_ns() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64
    }

    /// Handle an incoming capability invocation
    pub async fn handle_invoke(
        &self,
        cap_id: CapabilityId,
        request: InvokeRequest,
    ) -> Result<InvokeResponse, InvokeError> {
        let start = Instant::now();

        // 1. Get and validate capability
        let cap = {
            let caps = self.capabilities.read().await;
            caps.get(&cap_id)
                .cloned()
                .ok_or(InvokeError::CapabilityNotFound)?
        };

        let now = Self::current_time_ns();

        // Check expiration
        if cap.is_expired(now) {
            return Err(InvokeError::ValidationFailed(ValidationResult::Expired));
        }

        // Check revocation
        if cap.is_revoked() {
            return Err(InvokeError::ValidationFailed(ValidationResult::Revoked));
        }

        // Check quota
        if cap.quota.is_exhausted() {
            return Err(InvokeError::ValidationFailed(ValidationResult::QuotaExhausted));
        }

        // Check scope
        if !cap.scope.permits(&request.operation) {
            return Err(InvokeError::ValidationFailed(ValidationResult::ScopeViolation));
        }

        // 2. Log pre-invocation
        let _pre_hash = self
            .evidence_logger
            .log_pre_invoke(&cap, &request)
            .await
            .map_err(|e| InvokeError::EvidenceError(e.to_string()))?;

        // 3. Execute operation
        let executor_result = match cap.cap_type {
            CapabilityType::NetworkHttp | CapabilityType::NetworkTcp | CapabilityType::NetworkDns => {
                self.network_executor.execute(&cap, &request).await
            }
            CapabilityType::FileRead
            | CapabilityType::FileWrite
            | CapabilityType::FileDelete
            | CapabilityType::DirectoryList => {
                self.filesystem_executor.execute(&cap, &request).await
            }
            CapabilityType::SecretRead => {
                self.secrets_executor.execute(&cap, &request).await
            }
            _ => {
                return Err(InvokeError::UnsupportedOperation);
            }
        }
        .map_err(|e| InvokeError::ExecutionFailed(e.to_string()))?;

        // 4. Deduct budget
        self.deduct_budget(&cap_id, &executor_result.quota_consumed)
            .await?;

        // 5. Build response
        let response = InvokeResponse {
            result: executor_result.data,
            quota_consumed: executor_result.quota_consumed.clone(),
            evidence_hash: [0u8; 32], // Will be filled by post-invoke
        };

        // 6. Log post-invocation
        let evidence_hash = self
            .evidence_logger
            .log_post_invoke(&cap, &request, &response)
            .await
            .map_err(|e| InvokeError::EvidenceError(e.to_string()))?;

        let mut final_response = response;
        final_response.evidence_hash = evidence_hash;

        let elapsed = start.elapsed();
        debug!(
            capability_id = %cap_id,
            duration_ms = elapsed.as_millis(),
            "Invocation completed"
        );

        Ok(final_response)
    }

    /// Grant a capability to a capsule
    pub async fn grant_capability(
        &self,
        capsule_id: CapsuleId,
        grant: CapabilityGrant,
    ) -> Result<Capability, GrantError> {
        let now = Self::current_time_ns();

        // Create capability from grant
        let cap = Capability::from_grant(capsule_id, grant, now);
        let cap_id = cap.id;

        // Check policy limits
        {
            let capsule_caps = self.capsule_capabilities.read().await;
            if let Some(caps) = capsule_caps.get(&capsule_id) {
                if caps.len() >= self.config.general.max_capabilities_per_capsule {
                    return Err(GrantError::PolicyViolation(format!(
                        "Capsule already has {} capabilities (max {})",
                        caps.len(),
                        self.config.general.max_capabilities_per_capsule
                    )));
                }
            }
        }

        // Insert capability
        {
            let mut caps = self.capabilities.write().await;
            caps.insert(cap_id, cap.clone());
        }

        // Track by capsule
        {
            let mut capsule_caps = self.capsule_capabilities.write().await;
            capsule_caps
                .entry(capsule_id)
                .or_insert_with(Vec::new)
                .push(cap_id);
        }

        // Log grant
        self.evidence_logger
            .log_grant(&cap)
            .await
            .map_err(|e| GrantError::EvidenceError(e.to_string()))?;

        info!(
            capability_id = %cap_id,
            capsule_id = %capsule_id,
            capability_type = ?cap.cap_type,
            "Granted capability"
        );

        Ok(cap)
    }

    /// Revoke a capability
    pub async fn revoke(&self, cap_id: CapabilityId) -> Result<(), RevokeError> {
        // Get and remove capability
        let cap = {
            let mut caps = self.capabilities.write().await;
            caps.remove(&cap_id).ok_or(RevokeError::NotFound)?
        };

        // Check if already revoked
        if cap.revoked {
            return Err(RevokeError::AlreadyRevoked);
        }

        // Find and revoke all derived capabilities
        let derived_ids: Vec<CapabilityId> = {
            let caps = self.capabilities.read().await;
            caps.iter()
                .filter(|(_, c)| c.parent == Some(cap_id))
                .map(|(id, _)| *id)
                .collect()
        };

        for derived_id in derived_ids {
            // Recursive revocation
            let _ = Box::pin(self.revoke(derived_id)).await;
        }

        // Remove from capsule tracking
        {
            let mut capsule_caps = self.capsule_capabilities.write().await;
            if let Some(caps) = capsule_caps.get_mut(&cap.capsule_id) {
                caps.retain(|id| *id != cap_id);
            }
        }

        // Log revocation
        self.evidence_logger
            .log_revoke(&cap)
            .await
            .map_err(|e| RevokeError::EvidenceError(e.to_string()))?;

        info!(capability_id = %cap_id, "Revoked capability");

        Ok(())
    }

    /// Revoke all capabilities for a capsule
    pub async fn revoke_all(&self, capsule_id: CapsuleId) -> Result<usize, RevokeError> {
        let cap_ids: Vec<CapabilityId> = {
            let capsule_caps = self.capsule_capabilities.read().await;
            capsule_caps
                .get(&capsule_id)
                .cloned()
                .unwrap_or_default()
        };

        let count = cap_ids.len();
        for cap_id in cap_ids {
            let _ = self.revoke(cap_id).await;
        }

        info!(
            capsule_id = %capsule_id,
            count = count,
            "Revoked all capabilities for capsule"
        );

        Ok(count)
    }

    /// Deduct budget from a capability
    async fn deduct_budget(
        &self,
        cap_id: &CapabilityId,
        consumed: &QuotaConsumed,
    ) -> Result<(), BudgetError> {
        let mut caps = self.capabilities.write().await;

        let cap = caps
            .get_mut(cap_id)
            .ok_or(BudgetError::CapabilityNotFound)?;

        // Check if deduction would exhaust quota
        if cap.quota.would_exhaust(consumed) {
            warn!(
                capability_id = %cap_id,
                "Budget exhausted, marking capability for revocation"
            );
        }

        // Deduct
        cap.quota.deduct(consumed);

        // Auto-revoke if exhausted
        if cap.quota.is_exhausted() {
            cap.revoked = true;
        }

        Ok(())
    }

    /// Query remaining quota for a capability
    pub async fn query_quota(&self, cap_id: CapabilityId) -> Option<Quota> {
        let caps = self.capabilities.read().await;
        caps.get(&cap_id).map(|c| c.quota.clone())
    }

    /// Get capability by ID
    pub async fn get_capability(&self, cap_id: CapabilityId) -> Option<Capability> {
        let caps = self.capabilities.read().await;
        caps.get(&cap_id).cloned()
    }

    /// Run the proxy server
    pub async fn run(&self) -> Result<(), ProxyError> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
        let (message_tx, mut message_rx) =
            mpsc::channel::<(MessageEnvelope, mpsc::Sender<MessageEnvelope>)>(1000);

        // Start vsock listener
        let listener = VsockListener::bind(&self.config.vsock)
            .await
            .map_err(|e| ProxyError::Network(e.to_string()))?;

        info!(
            "Capability proxy running on {:?}",
            listener.local_addr()
        );

        // Spawn connection acceptor
        let message_tx_clone = message_tx.clone();
        let accept_handle = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok(conn) => {
                        let handler = ConnectionHandler::new(conn, message_tx_clone.clone());
                        tokio::spawn(handler.run());
                    }
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                    }
                }
            }
        });

        // Main message processing loop
        loop {
            tokio::select! {
                Some((envelope, response_tx)) = message_rx.recv() => {
                    let response = self.handle_message(envelope).await;
                    let _ = response_tx.send(response).await;
                }
                _ = shutdown_rx.recv() => {
                    info!("Shutdown signal received");
                    break;
                }
            }
        }

        accept_handle.abort();
        Ok(())
    }

    /// Handle an incoming message
    async fn handle_message(&self, envelope: MessageEnvelope) -> MessageEnvelope {
        match envelope.message_type {
            MessageType::Invoke => {
                match envelope.decode_payload::<InvokeRequest>() {
                    Ok(request) => {
                        match self.handle_invoke(envelope.capability_id, request).await {
                            Ok(response) => {
                                MessageEnvelope::invoke_response(
                                    envelope.sequence,
                                    envelope.capability_id,
                                    &response,
                                )
                                .unwrap_or_else(|_| self.error_envelope(
                                    envelope.sequence,
                                    envelope.capability_id,
                                    "SERIALIZATION_ERROR",
                                    "Failed to serialize response",
                                ))
                            }
                            Err(e) => self.error_envelope(
                                envelope.sequence,
                                envelope.capability_id,
                                "INVOKE_ERROR",
                                &e.to_string(),
                            ),
                        }
                    }
                    Err(e) => self.error_envelope(
                        envelope.sequence,
                        envelope.capability_id,
                        "PARSE_ERROR",
                        &e.to_string(),
                    ),
                }
            }
            MessageType::QueryQuota => {
                match self.query_quota(envelope.capability_id).await {
                    Some(quota) => {
                        let payload = crate::wire::QuotaResponsePayload {
                            quota,
                            valid: true,
                        };
                        let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
                        MessageEnvelope::new(
                            envelope.sequence,
                            envelope.capability_id,
                            MessageType::QuotaResult,
                            bytes::Bytes::from(payload_bytes),
                        )
                    }
                    None => self.error_envelope(
                        envelope.sequence,
                        envelope.capability_id,
                        "NOT_FOUND",
                        "Capability not found",
                    ),
                }
            }
            MessageType::Revoke => {
                match self.revoke(envelope.capability_id).await {
                    Ok(()) => {
                        let payload = crate::wire::RevokeResponsePayload { revoked_count: 1 };
                        let payload_bytes = serde_json::to_vec(&payload).unwrap_or_default();
                        MessageEnvelope::new(
                            envelope.sequence,
                            envelope.capability_id,
                            MessageType::RevokeResult,
                            bytes::Bytes::from(payload_bytes),
                        )
                    }
                    Err(e) => self.error_envelope(
                        envelope.sequence,
                        envelope.capability_id,
                        "REVOKE_ERROR",
                        &e.to_string(),
                    ),
                }
            }
            _ => self.error_envelope(
                envelope.sequence,
                envelope.capability_id,
                "UNSUPPORTED",
                "Unsupported message type",
            ),
        }
    }

    /// Create an error response envelope
    fn error_envelope(
        &self,
        sequence: u64,
        cap_id: CapabilityId,
        code: &str,
        message: &str,
    ) -> MessageEnvelope {
        let error = ErrorPayload::new(code, message);
        MessageEnvelope::error_response(sequence, cap_id, &error)
            .unwrap_or_else(|_| MessageEnvelope::new(
                sequence,
                cap_id,
                MessageType::Error,
                bytes::Bytes::new(),
            ))
    }

    /// Shutdown the proxy
    pub async fn shutdown(&self) {
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(()).await;
        }
    }

    /// Get proxy statistics
    pub async fn stats(&self) -> ProxyStats {
        let caps = self.capabilities.read().await;
        ProxyStats {
            active_capabilities: caps.len(),
            merkle_root: self.evidence_logger.merkle_root().await,
        }
    }
}

/// Proxy statistics
#[derive(Debug, Clone)]
pub struct ProxyStats {
    /// Number of active capabilities
    pub active_capabilities: usize,
    /// Current Merkle root of evidence log
    pub merkle_root: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CapabilityScope, Rights};

    fn create_test_config() -> ProxyConfig {
        let mut config = ProxyConfig::default();
        config.evidence.enabled = false; // Disable evidence for tests
        config.vsock.tcp_fallback = true;
        config.vsock.tcp_address = "127.0.0.1:0".to_string();
        config
    }

    #[tokio::test]
    async fn test_proxy_grant_capability() {
        let config = create_test_config();
        let proxy = CapabilityProxy::new(config).await.unwrap();

        let capsule_id = CapsuleId::generate();
        let grant = CapabilityGrant {
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Clock,
            rights: Rights::default(),
            quota: Quota::new(100, 1_000_000, 60_000_000_000),
            duration_ns: 3600_000_000_000,
        };

        let cap = proxy.grant_capability(capsule_id, grant).await.unwrap();
        assert_eq!(cap.capsule_id, capsule_id);
        assert_eq!(cap.cap_type, CapabilityType::NetworkHttp);
    }

    #[tokio::test]
    async fn test_proxy_revoke_capability() {
        let config = create_test_config();
        let proxy = CapabilityProxy::new(config).await.unwrap();

        let capsule_id = CapsuleId::generate();
        let grant = CapabilityGrant {
            cap_type: CapabilityType::FileRead,
            scope: CapabilityScope::Clock,
            rights: Rights::default(),
            quota: Quota::new(100, 1_000_000, 60_000_000_000),
            duration_ns: 3600_000_000_000,
        };

        let cap = proxy.grant_capability(capsule_id, grant).await.unwrap();
        let cap_id = cap.id;

        // Revoke should succeed
        assert!(proxy.revoke(cap_id).await.is_ok());

        // Second revoke should fail
        assert!(proxy.revoke(cap_id).await.is_err());
    }

    #[tokio::test]
    async fn test_proxy_revoke_all() {
        let config = create_test_config();
        let proxy = CapabilityProxy::new(config).await.unwrap();

        let capsule_id = CapsuleId::generate();

        // Grant multiple capabilities
        for _ in 0..3 {
            let grant = CapabilityGrant {
                cap_type: CapabilityType::NetworkHttp,
                scope: CapabilityScope::Clock,
                rights: Rights::default(),
                quota: Quota::new(100, 1_000_000, 60_000_000_000),
                duration_ns: 3600_000_000_000,
            };
            proxy.grant_capability(capsule_id, grant).await.unwrap();
        }

        // Revoke all
        let count = proxy.revoke_all(capsule_id).await.unwrap();
        assert_eq!(count, 3);

        // Stats should show 0 capabilities
        let stats = proxy.stats().await;
        assert_eq!(stats.active_capabilities, 0);
    }

    #[tokio::test]
    async fn test_proxy_query_quota() {
        let config = create_test_config();
        let proxy = CapabilityProxy::new(config).await.unwrap();

        let capsule_id = CapsuleId::generate();
        let grant = CapabilityGrant {
            cap_type: CapabilityType::NetworkHttp,
            scope: CapabilityScope::Clock,
            rights: Rights::default(),
            quota: Quota::new(100, 1_000_000, 60_000_000_000),
            duration_ns: 3600_000_000_000,
        };

        let cap = proxy.grant_capability(capsule_id, grant).await.unwrap();

        let quota = proxy.query_quota(cap.id).await.unwrap();
        assert_eq!(quota.max_invocations, 100);
        assert_eq!(quota.used_invocations, 0);
    }
}
