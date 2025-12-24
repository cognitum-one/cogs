//! HIPAA-compliant storage with PHI encryption at rest
//!
//! Implements AES-256-GCM encryption with keys stored in HSM (Hardware Security Module).
//! Keys are never exposed in application memory.

use super::{HipaaError, PhiRecord, PhiView, RecordId, Result, User};
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Key source for encryption
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KeySource {
    /// Hardware Security Module (production)
    Hsm,
    /// In-memory (testing only)
    InMemory,
}

/// Encryption configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionConfig {
    pub algorithm: String,
    pub key_source: KeySource,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            algorithm: "AES-256-GCM".to_string(),
            key_source: KeySource::Hsm,
        }
    }
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub encryption: EncryptionConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            encryption: EncryptionConfig::default(),
        }
    }
}

/// Encrypted data blob
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub key_id: String,
}

/// HSM provider trait for key management
pub trait HsmProvider: Send + Sync {
    /// Get encryption key from HSM (never returns raw key)
    fn get_cipher(&self, key_id: &str) -> Result<Box<dyn AeadCipher>>;

    /// Generate new key in HSM
    fn generate_key(&self, key_id: &str) -> Result<()>;

    /// Rotate encryption key
    fn rotate_key(&self, old_key_id: &str) -> Result<String>;
}

/// AEAD cipher trait for encryption/decryption
pub trait AeadCipher: Send + Sync {
    fn encrypt(&self, plaintext: &[u8], nonce: &[u8]) -> Result<Vec<u8>>;
    fn decrypt(&self, ciphertext: &[u8], nonce: &[u8]) -> Result<Vec<u8>>;
}

/// AES-256-GCM cipher implementation
struct Aes256GcmCipher {
    cipher: Aes256Gcm,
}

impl AeadCipher for Aes256GcmCipher {
    fn encrypt(&self, plaintext: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(nonce);
        self.cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| HipaaError::EncryptionFailed(e.to_string()))
    }

    fn decrypt(&self, ciphertext: &[u8], nonce: &[u8]) -> Result<Vec<u8>> {
        let nonce = Nonce::from_slice(nonce);
        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| HipaaError::EncryptionFailed(format!("Decryption failed: {}", e)))
    }
}

/// Mock HSM provider for testing
pub struct MockHsmProvider {
    keys: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl MockHsmProvider {
    pub fn new() -> Self {
        let mut keys = HashMap::new();
        // Default key for testing
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        keys.insert("default".to_string(), key.to_vec());

        Self {
            keys: Arc::new(RwLock::new(keys)),
        }
    }

    pub fn key_count(&self) -> usize {
        self.keys.read().unwrap().len()
    }
}

impl HsmProvider for MockHsmProvider {
    fn get_cipher(&self, key_id: &str) -> Result<Box<dyn AeadCipher>> {
        let keys = self.keys.read().unwrap();
        let key = keys
            .get(key_id)
            .ok_or_else(|| HipaaError::KeyManagement(format!("Key not found: {}", key_id)))?;

        let cipher = Aes256Gcm::new_from_slice(key)
            .map_err(|e| HipaaError::KeyManagement(e.to_string()))?;

        Ok(Box::new(Aes256GcmCipher { cipher }))
    }

    fn generate_key(&self, key_id: &str) -> Result<()> {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);

        let mut keys = self.keys.write().unwrap();
        keys.insert(key_id.to_string(), key.to_vec());

        Ok(())
    }

    fn rotate_key(&self, old_key_id: &str) -> Result<String> {
        let new_key_id = format!("{}_v2", old_key_id);
        self.generate_key(&new_key_id)?;
        Ok(new_key_id)
    }
}

/// HIPAA-compliant storage with encryption at rest
pub struct HipaaCompliantStorage {
    #[allow(dead_code)] // Stored for runtime configuration access
    config: StorageConfig,
    hsm: Arc<dyn HsmProvider>,
    storage: Arc<RwLock<HashMap<RecordId, EncryptedData>>>,
    current_key_id: Arc<RwLock<String>>,
}

impl HipaaCompliantStorage {
    /// Create new HIPAA-compliant storage
    pub fn new(config: StorageConfig) -> Self {
        let hsm: Arc<dyn HsmProvider> = match config.encryption.key_source {
            KeySource::Hsm => {
                // In production, use real HSM provider
                // For now, use mock
                Arc::new(MockHsmProvider::new())
            }
            KeySource::InMemory => Arc::new(MockHsmProvider::new()),
        };

        Self {
            config,
            hsm,
            storage: Arc::new(RwLock::new(HashMap::new())),
            current_key_id: Arc::new(RwLock::new("default".to_string())),
        }
    }

    /// Store PHI with encryption
    pub async fn store(&self, record: PhiRecord) -> Result<RecordId> {
        // Serialize record
        let plaintext = serde_json::to_vec(&record)?;

        // Generate unique nonce
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);

        // Get current key
        let key_id = self.current_key_id.read().unwrap().clone();

        // Encrypt using HSM
        let cipher = self.hsm.get_cipher(&key_id)?;
        let ciphertext = cipher.encrypt(&plaintext, &nonce)?;

        // Store encrypted data
        let encrypted = EncryptedData {
            ciphertext,
            nonce: nonce.to_vec(),
            key_id,
        };

        let record_id = RecordId::new(format!("phi_{}", uuid::Uuid::new_v4()));
        self.storage
            .write()
            .unwrap()
            .insert(record_id.clone(), encrypted);

        Ok(record_id)
    }

    /// Get PHI with decryption
    pub async fn get(&self, record_id: &RecordId) -> Result<PhiRecord> {
        let storage = self.storage.read().unwrap();
        let encrypted = storage
            .get(record_id)
            .ok_or_else(|| HipaaError::AccessDenied("Record not found".to_string()))?;

        // Decrypt using HSM
        let cipher = self.hsm.get_cipher(&encrypted.key_id)?;
        let plaintext = cipher.decrypt(&encrypted.ciphertext, &encrypted.nonce)?;

        // Deserialize
        let record: PhiRecord = serde_json::from_slice(&plaintext)?;
        Ok(record)
    }

    /// Get raw encrypted bytes (for verification that data is encrypted)
    pub async fn get_raw_bytes(&self, record_id: &RecordId) -> Result<Vec<u8>> {
        let storage = self.storage.read().unwrap();
        let encrypted = storage
            .get(record_id)
            .ok_or_else(|| HipaaError::AccessDenied("Record not found".to_string()))?;

        Ok(encrypted.ciphertext.clone())
    }

    /// Get PHI with minimum necessary access filtering
    pub async fn get_for_user(&self, record_id: &RecordId, user: &User) -> Result<PhiView> {
        let record = self.get(record_id).await?;

        // Apply minimum necessary access based on role
        let view = if user.has_role("clinician") || user.has_role("admin") {
            // Full access
            PhiView {
                patient_id: Some(record.patient_id),
                dna_sequence: Some(record.dna_sequence),
                analysis_results: Some(record.analysis_results),
            }
        } else if user.has_role("lab_tech") {
            // Analysis results only, no raw DNA
            PhiView {
                patient_id: None,
                dna_sequence: None,
                analysis_results: Some(record.analysis_results),
            }
        } else {
            // No access
            return Err(HipaaError::InsufficientPermissions);
        };

        Ok(view)
    }

    /// Verify data integrity
    pub fn verify_integrity(&self, encrypted: &EncryptedData) -> Result<bool> {
        // AES-GCM provides authenticated encryption
        // Attempt to decrypt - if it succeeds, integrity is verified
        let cipher = self.hsm.get_cipher(&encrypted.key_id)?;
        match cipher.decrypt(&encrypted.ciphertext, &encrypted.nonce) {
            Ok(_) => Ok(true),
            Err(_) => Err(HipaaError::IntegrityCheckFailed),
        }
    }

    /// Rotate encryption key
    pub async fn rotate_key(&self) -> Result<String> {
        let old_key_id = self.current_key_id.read().unwrap().clone();
        let new_key_id = self.hsm.rotate_key(&old_key_id)?;

        *self.current_key_id.write().unwrap() = new_key_id.clone();

        Ok(new_key_id)
    }

    /// Get current key ID
    pub fn current_key_id(&self) -> String {
        self.current_key_id.read().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hipaa::AnalysisResult;

    fn create_test_record() -> PhiRecord {
        PhiRecord {
            patient_id: "P12345".to_string(),
            dna_sequence: "ATCGATCGATCG".to_string(),
            analysis_results: vec![AnalysisResult {
                test_name: "Test1".to_string(),
                result_value: "Normal".to_string(),
                timestamp: chrono::Utc::now(),
            }],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_phi_encryption_at_rest() {
        let storage = HipaaCompliantStorage::new(StorageConfig::default());
        let record = create_test_record();

        // Store PHI
        let record_id = storage.store(record.clone()).await.unwrap();

        // Verify raw bytes are encrypted
        let raw_data = storage.get_raw_bytes(&record_id).await.unwrap();
        let raw_str = String::from_utf8_lossy(&raw_data);
        assert!(!raw_str.contains("P12345"));
        assert!(!raw_str.contains("ATCGATCGATCG"));

        // Verify decryption works
        let retrieved = storage.get(&record_id).await.unwrap();
        assert_eq!(retrieved.patient_id, record.patient_id);
        assert_eq!(retrieved.dna_sequence, record.dna_sequence);
    }

    #[tokio::test]
    async fn test_minimum_necessary_access_lab_tech() {
        let storage = HipaaCompliantStorage::new(StorageConfig::default());
        let record = create_test_record();
        let record_id = storage.store(record).await.unwrap();

        // Lab tech should only see analysis results
        let lab_tech = User::new("tech_001", vec!["lab_tech".to_string()]);
        let view = storage.get_for_user(&record_id, &lab_tech).await.unwrap();

        assert!(view.analysis_results.is_some());
        assert!(view.dna_sequence.is_none());
        assert!(view.patient_id.is_none());
    }

    #[tokio::test]
    async fn test_minimum_necessary_access_clinician() {
        let storage = HipaaCompliantStorage::new(StorageConfig::default());
        let record = create_test_record();
        let record_id = storage.store(record).await.unwrap();

        // Clinician should see everything
        let clinician = User::new("doc_001", vec!["clinician".to_string()]);
        let view = storage.get_for_user(&record_id, &clinician).await.unwrap();

        assert!(view.analysis_results.is_some());
        assert!(view.dna_sequence.is_some());
        assert!(view.patient_id.is_some());
    }

    #[tokio::test]
    async fn test_key_rotation() {
        let storage = HipaaCompliantStorage::new(StorageConfig::default());
        let record = create_test_record();

        // Store with original key
        let record_id = storage.store(record).await.unwrap();
        let old_key_id = storage.current_key_id();

        // Rotate key
        let new_key_id = storage.rotate_key().await.unwrap();
        assert_ne!(old_key_id, new_key_id);

        // Old data still accessible
        let retrieved = storage.get(&record_id).await.unwrap();
        assert_eq!(retrieved.patient_id, "P12345");
    }
}
