//! Evidence logging for the capability proxy.
//!
//! Implements the evidence chain per ADR-006, logging all capability
//! invocations, grants, and revocations for audit and replay.

use crate::config::EvidenceConfig;
use crate::types::{Capability, CapabilityId, InvokeRequest, InvokeResponse, QuotaConsumed};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, BufWriter};
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Evidence logger for capability operations
pub struct EvidenceLogger {
    /// Configuration
    config: EvidenceConfig,
    /// Current log file writer
    writer: Arc<Mutex<Option<BufWriter<File>>>>,
    /// Merkle tree for integrity
    merkle_tree: Arc<RwLock<MerkleTree>>,
    /// Current sequence number
    sequence: Arc<std::sync::atomic::AtomicU64>,
    /// Pending writes channel
    write_tx: mpsc::Sender<EvidenceEntry>,
    /// Background writer handle
    _writer_handle: Option<tokio::task::JoinHandle<()>>,
}

impl EvidenceLogger {
    /// Create a new evidence logger
    pub async fn new(config: &EvidenceConfig) -> Result<Self, std::io::Error> {
        // Create evidence directory if needed
        if config.enabled {
            tokio::fs::create_dir_all(&config.path).await?;
        }

        let (write_tx, write_rx) = mpsc::channel(1000);
        let merkle_tree = Arc::new(RwLock::new(MerkleTree::new()));

        let logger = Self {
            config: config.clone(),
            writer: Arc::new(Mutex::new(None)),
            merkle_tree: merkle_tree.clone(),
            sequence: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            write_tx,
            _writer_handle: None,
        };

        if config.enabled {
            logger.open_log_file().await?;
        }

        // Start background writer
        let writer_clone = logger.writer.clone();
        let config_clone = config.clone();
        let merkle_clone = merkle_tree.clone();
        let handle = tokio::spawn(async move {
            Self::background_writer(write_rx, writer_clone, config_clone, merkle_clone).await;
        });

        Ok(Self {
            _writer_handle: Some(handle),
            ..logger
        })
    }

    /// Open or rotate log file
    async fn open_log_file(&self) -> Result<(), std::io::Error> {
        let filename = format!(
            "evidence-{}.jsonl",
            Utc::now().format("%Y%m%d-%H%M%S")
        );
        let path = self.config.path.join(&filename);

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await?;

        let mut writer = self.writer.lock().await;
        *writer = Some(BufWriter::new(file));

        info!("Opened evidence log: {:?}", path);
        Ok(())
    }

    /// Background writer task
    async fn background_writer(
        mut rx: mpsc::Receiver<EvidenceEntry>,
        writer: Arc<Mutex<Option<BufWriter<File>>>>,
        config: EvidenceConfig,
        merkle: Arc<RwLock<MerkleTree>>,
    ) {
        let mut batch = Vec::with_capacity(100);
        let flush_interval = config.flush_interval;

        loop {
            tokio::select! {
                entry = rx.recv() => {
                    match entry {
                        Some(entry) => {
                            batch.push(entry);
                            if batch.len() >= 100 {
                                Self::flush_batch(&writer, &merkle, &mut batch).await;
                            }
                        }
                        None => {
                            // Channel closed, flush remaining and exit
                            Self::flush_batch(&writer, &merkle, &mut batch).await;
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(flush_interval) => {
                    if !batch.is_empty() {
                        Self::flush_batch(&writer, &merkle, &mut batch).await;
                    }
                }
            }
        }
    }

    /// Flush a batch of entries to disk
    async fn flush_batch(
        writer: &Arc<Mutex<Option<BufWriter<File>>>>,
        merkle: &Arc<RwLock<MerkleTree>>,
        batch: &mut Vec<EvidenceEntry>,
    ) {
        if batch.is_empty() {
            return;
        }

        let mut writer_guard = writer.lock().await;
        if let Some(ref mut w) = *writer_guard {
            for entry in batch.drain(..) {
                // Add to Merkle tree
                let hash = entry.compute_hash();
                {
                    let mut tree = merkle.write().await;
                    tree.append(hash);
                }

                // Write JSON line
                if let Ok(json) = serde_json::to_string(&entry) {
                    if let Err(e) = w.write_all(json.as_bytes()).await {
                        error!("Failed to write evidence entry: {}", e);
                    }
                    if let Err(e) = w.write_all(b"\n").await {
                        error!("Failed to write newline: {}", e);
                    }
                }
            }

            if let Err(e) = w.flush().await {
                error!("Failed to flush evidence log: {}", e);
            }
        }
    }

    /// Get next sequence number
    fn next_sequence(&self) -> u64 {
        self.sequence
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Log pre-invocation event
    pub async fn log_pre_invoke(
        &self,
        cap: &Capability,
        request: &InvokeRequest,
    ) -> Result<[u8; 32], std::io::Error> {
        if !self.config.enabled {
            return Ok([0u8; 32]);
        }

        let entry = EvidenceEntry {
            sequence: self.next_sequence(),
            timestamp: Utc::now(),
            event_type: EvidenceEventType::PreInvoke,
            capability_id: cap.id,
            capsule_id: Some(cap.capsule_id.0),
            capability_type: Some(cap.cap_type as u16),
            request_hash: Some(Self::hash_request(request)),
            response_hash: None,
            quota_consumed: None,
            error: None,
            metadata: None,
        };

        let hash = entry.compute_hash();
        self.write_tx.send(entry).await.ok();

        Ok(hash)
    }

    /// Log post-invocation event
    pub async fn log_post_invoke(
        &self,
        cap: &Capability,
        request: &InvokeRequest,
        response: &InvokeResponse,
    ) -> Result<[u8; 32], std::io::Error> {
        if !self.config.enabled {
            return Ok([0u8; 32]);
        }

        let entry = EvidenceEntry {
            sequence: self.next_sequence(),
            timestamp: Utc::now(),
            event_type: EvidenceEventType::PostInvoke,
            capability_id: cap.id,
            capsule_id: Some(cap.capsule_id.0),
            capability_type: Some(cap.cap_type as u16),
            request_hash: Some(Self::hash_request(request)),
            response_hash: Some(Self::hash_response(response)),
            quota_consumed: Some(response.quota_consumed.clone()),
            error: None,
            metadata: None,
        };

        let hash = entry.compute_hash();
        self.write_tx.send(entry).await.ok();

        Ok(hash)
    }

    /// Log capability grant
    pub async fn log_grant(&self, cap: &Capability) -> Result<[u8; 32], std::io::Error> {
        if !self.config.enabled {
            return Ok([0u8; 32]);
        }

        let entry = EvidenceEntry {
            sequence: self.next_sequence(),
            timestamp: Utc::now(),
            event_type: EvidenceEventType::Grant,
            capability_id: cap.id,
            capsule_id: Some(cap.capsule_id.0),
            capability_type: Some(cap.cap_type as u16),
            request_hash: None,
            response_hash: None,
            quota_consumed: None,
            error: None,
            metadata: Some(serde_json::json!({
                "expires_at": cap.expires_at,
                "rights": cap.rights.0,
                "parent": cap.parent.map(|p| p.0),
            })),
        };

        let hash = entry.compute_hash();
        self.write_tx.send(entry).await.ok();

        info!(
            "Logged capability grant: {} for capsule {:?}",
            cap.id, cap.capsule_id
        );

        Ok(hash)
    }

    /// Log capability revocation
    pub async fn log_revoke(&self, cap: &Capability) -> Result<[u8; 32], std::io::Error> {
        if !self.config.enabled {
            return Ok([0u8; 32]);
        }

        let entry = EvidenceEntry {
            sequence: self.next_sequence(),
            timestamp: Utc::now(),
            event_type: EvidenceEventType::Revoke,
            capability_id: cap.id,
            capsule_id: Some(cap.capsule_id.0),
            capability_type: Some(cap.cap_type as u16),
            request_hash: None,
            response_hash: None,
            quota_consumed: None,
            error: None,
            metadata: None,
        };

        let hash = entry.compute_hash();
        self.write_tx.send(entry).await.ok();

        info!("Logged capability revocation: {}", cap.id);

        Ok(hash)
    }

    /// Log an error event
    pub async fn log_error(
        &self,
        cap_id: CapabilityId,
        error_code: &str,
        error_message: &str,
    ) -> Result<[u8; 32], std::io::Error> {
        if !self.config.enabled {
            return Ok([0u8; 32]);
        }

        let entry = EvidenceEntry {
            sequence: self.next_sequence(),
            timestamp: Utc::now(),
            event_type: EvidenceEventType::Error,
            capability_id: cap_id,
            capsule_id: None,
            capability_type: None,
            request_hash: None,
            response_hash: None,
            quota_consumed: None,
            error: Some(ErrorInfo {
                code: error_code.to_string(),
                message: error_message.to_string(),
            }),
            metadata: None,
        };

        let hash = entry.compute_hash();
        self.write_tx.send(entry).await.ok();

        Ok(hash)
    }

    /// Get current Merkle root
    pub async fn merkle_root(&self) -> [u8; 32] {
        self.merkle_tree.read().await.root()
    }

    /// Get inclusion proof for an entry
    pub async fn inclusion_proof(&self, sequence: u64) -> Option<Vec<[u8; 32]>> {
        let tree = self.merkle_tree.read().await;
        if sequence as usize >= tree.leaf_count() {
            return None;
        }
        Some(tree.inclusion_proof(sequence as usize))
    }

    /// Hash an invoke request
    fn hash_request(request: &InvokeRequest) -> [u8; 32] {
        let json = serde_json::to_vec(request).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&json);
        hasher.finalize().into()
    }

    /// Hash an invoke response
    fn hash_response(response: &InvokeResponse) -> [u8; 32] {
        let json = serde_json::to_vec(response).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&json);
        hasher.finalize().into()
    }
}

/// Evidence entry type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceEventType {
    /// Before invocation
    PreInvoke,
    /// After invocation
    PostInvoke,
    /// Capability granted
    Grant,
    /// Capability revoked
    Revoke,
    /// Error occurred
    Error,
}

/// Error information in evidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// Error code
    pub code: String,
    /// Error message
    pub message: String,
}

/// Evidence entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceEntry {
    /// Sequence number
    pub sequence: u64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Event type
    pub event_type: EvidenceEventType,
    /// Capability ID
    pub capability_id: CapabilityId,
    /// Capsule ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capsule_id: Option<[u8; 16]>,
    /// Capability type code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_type: Option<u16>,
    /// Hash of the request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_hash: Option<[u8; 32]>,
    /// Hash of the response
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_hash: Option<[u8; 32]>,
    /// Quota consumed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quota_consumed: Option<QuotaConsumed>,
    /// Error info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorInfo>,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl EvidenceEntry {
    /// Compute SHA256 hash of this entry
    pub fn compute_hash(&self) -> [u8; 32] {
        let json = serde_json::to_vec(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&json);
        hasher.finalize().into()
    }
}

/// Merkle tree for evidence integrity
pub struct MerkleTree {
    /// Leaf hashes
    leaves: Vec<[u8; 32]>,
    /// Internal nodes (flattened by level)
    nodes: Vec<Vec<[u8; 32]>>,
}

impl MerkleTree {
    /// Create a new empty Merkle tree
    pub fn new() -> Self {
        Self {
            leaves: Vec::new(),
            nodes: Vec::new(),
        }
    }

    /// Number of leaves
    pub fn leaf_count(&self) -> usize {
        self.leaves.len()
    }

    /// Append a leaf and rebuild affected nodes
    pub fn append(&mut self, leaf_hash: [u8; 32]) {
        self.leaves.push(leaf_hash);
        self.rebuild();
    }

    /// Get the current root hash
    pub fn root(&self) -> [u8; 32] {
        if self.nodes.is_empty() {
            if self.leaves.is_empty() {
                return [0u8; 32];
            }
            if self.leaves.len() == 1 {
                return self.leaves[0];
            }
        }
        self.nodes.last().and_then(|l| l.first().copied()).unwrap_or([0u8; 32])
    }

    /// Generate inclusion proof for a leaf
    pub fn inclusion_proof(&self, leaf_index: usize) -> Vec<[u8; 32]> {
        if leaf_index >= self.leaves.len() {
            return Vec::new();
        }

        let mut proof = Vec::new();
        let mut idx = leaf_index;
        let mut current_level = &self.leaves[..];

        while current_level.len() > 1 {
            let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            if sibling_idx < current_level.len() {
                proof.push(current_level[sibling_idx]);
            }
            idx /= 2;

            // Move to next level
            let level_idx = self.find_level_for_size(current_level.len());
            if level_idx < self.nodes.len() {
                current_level = &self.nodes[level_idx][..];
            } else {
                break;
            }
        }

        proof
    }

    /// Verify an inclusion proof
    pub fn verify_inclusion(
        leaf_hash: &[u8; 32],
        leaf_index: usize,
        proof: &[[u8; 32]],
        root: &[u8; 32],
    ) -> bool {
        let mut computed = *leaf_hash;
        let mut idx = leaf_index;

        for sibling in proof {
            computed = if idx % 2 == 0 {
                Self::hash_pair(&computed, sibling)
            } else {
                Self::hash_pair(sibling, &computed)
            };
            idx /= 2;
        }

        &computed == root
    }

    /// Rebuild the tree from leaves
    fn rebuild(&mut self) {
        self.nodes.clear();

        if self.leaves.len() <= 1 {
            return;
        }

        let mut current_level: Vec<[u8; 32]> = self.leaves.clone();

        while current_level.len() > 1 {
            let mut next_level = Vec::with_capacity((current_level.len() + 1) / 2);

            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    Self::hash_pair(&chunk[0], &chunk[1])
                } else {
                    chunk[0]
                };
                next_level.push(hash);
            }

            self.nodes.push(next_level.clone());
            current_level = next_level;
        }
    }

    /// Hash two nodes together
    fn hash_pair(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(left);
        hasher.update(right);
        hasher.finalize().into()
    }

    /// Find which level corresponds to a given size
    fn find_level_for_size(&self, size: usize) -> usize {
        for (i, level) in self.nodes.iter().enumerate() {
            if level.len() == (size + 1) / 2 {
                return i;
            }
        }
        self.nodes.len()
    }
}

impl Default for MerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_tree_single_leaf() {
        let mut tree = MerkleTree::new();
        let leaf = [1u8; 32];
        tree.append(leaf);
        assert_eq!(tree.root(), leaf);
    }

    #[test]
    fn test_merkle_tree_multiple_leaves() {
        let mut tree = MerkleTree::new();
        tree.append([1u8; 32]);
        tree.append([2u8; 32]);
        tree.append([3u8; 32]);
        tree.append([4u8; 32]);

        let root = tree.root();
        assert_ne!(root, [0u8; 32]);
        assert_eq!(tree.leaf_count(), 4);
    }

    #[test]
    fn test_merkle_inclusion_proof() {
        let mut tree = MerkleTree::new();
        let leaves: Vec<[u8; 32]> = (0..8).map(|i| [i as u8; 32]).collect();

        for leaf in &leaves {
            tree.append(*leaf);
        }

        let root = tree.root();

        for (i, leaf) in leaves.iter().enumerate() {
            let proof = tree.inclusion_proof(i);
            assert!(
                MerkleTree::verify_inclusion(leaf, i, &proof, &root),
                "Proof failed for leaf {}",
                i
            );
        }
    }

    #[test]
    fn test_evidence_entry_hash() {
        let entry = EvidenceEntry {
            sequence: 0,
            timestamp: Utc::now(),
            event_type: EvidenceEventType::Grant,
            capability_id: CapabilityId::generate(),
            capsule_id: None,
            capability_type: None,
            request_hash: None,
            response_hash: None,
            quota_consumed: None,
            error: None,
            metadata: None,
        };

        let hash1 = entry.compute_hash();
        let hash2 = entry.compute_hash();
        assert_eq!(hash1, hash2);
    }

    #[tokio::test]
    async fn test_evidence_logger_disabled() {
        let config = EvidenceConfig {
            enabled: false,
            ..Default::default()
        };

        let logger = EvidenceLogger::new(&config).await.unwrap();
        let hash = logger.log_grant(&Capability {
            id: CapabilityId::generate(),
            capsule_id: crate::types::CapsuleId::generate(),
            cap_type: crate::types::CapabilityType::NetworkHttp,
            scope: crate::types::CapabilityScope::Clock,
            rights: crate::types::Rights::default(),
            quota: crate::types::Quota::default(),
            expires_at: 0,
            parent: None,
            proof: crate::types::CapabilityProof::placeholder(),
            revoked: false,
        }).await.unwrap();

        assert_eq!(hash, [0u8; 32]);
    }
}
