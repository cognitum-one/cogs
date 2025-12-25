//! Unit tests for HIPAA-compliant storage

use cognitum::hipaa::*;

fn create_test_record() -> PhiRecord {
    PhiRecord {
        patient_id: "P12345".to_string(),
        dna_sequence: "ATCGATCGATCGATCG".to_string(),
        analysis_results: vec![AnalysisResult {
            test_name: "Genomic Analysis".to_string(),
            result_value: "Normal".to_string(),
            timestamp: chrono::Utc::now(),
        }],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    }
}

#[tokio::test]
async fn phi_is_encrypted_at_rest() {
    let storage = HipaaCompliantStorage::new(StorageConfig {
        encryption: EncryptionConfig {
            algorithm: "AES-256-GCM".to_string(),
            key_source: KeySource::Hsm,
        },
    });

    let phi_record = create_test_record();

    // Store PHI
    let record_id = storage.store(phi_record.clone()).await.unwrap();

    // Raw storage should be encrypted (not readable)
    let raw_data = storage.get_raw_bytes(&record_id).await.unwrap();
    let raw_str = String::from_utf8_lossy(&raw_data);

    assert!(!raw_str.contains("P12345"));
    assert!(!raw_str.contains("ATCGATCGATCGATCG"));

    // Decrypted retrieval works
    let retrieved = storage.get(&record_id).await.unwrap();
    assert_eq!(retrieved.patient_id, phi_record.patient_id);
    assert_eq!(retrieved.dna_sequence, phi_record.dna_sequence);
}

#[tokio::test]
async fn encryption_uses_unique_nonces() {
    let storage = HipaaCompliantStorage::new(StorageConfig::default());
    let record = create_test_record();

    // Store same record twice
    let id1 = storage.store(record.clone()).await.unwrap();
    let id2 = storage.store(record.clone()).await.unwrap();

    // Raw ciphertexts should be different (due to unique nonces)
    let raw1 = storage.get_raw_bytes(&id1).await.unwrap();
    let raw2 = storage.get_raw_bytes(&id2).await.unwrap();

    assert_ne!(raw1, raw2);
}

#[tokio::test]
async fn minimum_necessary_access_lab_tech() {
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
async fn minimum_necessary_access_clinician() {
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
async fn unauthorized_user_denied_access() {
    let storage = HipaaCompliantStorage::new(StorageConfig::default());
    let record = create_test_record();
    let record_id = storage.store(record).await.unwrap();

    // User with no valid role
    let unauthorized = User::new("user_001", vec!["guest".to_string()]);
    let result = storage.get_for_user(&record_id, &unauthorized).await;

    assert!(matches!(result, Err(HipaaError::InsufficientPermissions)));
}

#[tokio::test]
async fn key_rotation_maintains_data_accessibility() {
    let storage = HipaaCompliantStorage::new(StorageConfig::default());
    let record = create_test_record();

    // Encrypt with current key
    let record_id = storage.store(record.clone()).await.unwrap();
    let old_key_id = storage.current_key_id();

    // Rotate key
    let new_key_id = storage.rotate_key().await.unwrap();
    assert_ne!(new_key_id, old_key_id);

    // Old data can still be decrypted
    let retrieved = storage.get(&record_id).await.unwrap();
    assert_eq!(retrieved.patient_id, record.patient_id);

    // New encryptions use new key
    let new_record = create_test_record();
    let new_record_id = storage.store(new_record).await.unwrap();

    // Both old and new records accessible
    assert!(storage.get(&record_id).await.is_ok());
    assert!(storage.get(&new_record_id).await.is_ok());
}

#[tokio::test]
async fn data_integrity_verification() {
    let storage = HipaaCompliantStorage::new(StorageConfig::default());
    let record = create_test_record();

    let record_id = storage.store(record).await.unwrap();

    // Get encrypted data
    let encrypted = {
        let storage_map = storage.storage.read().unwrap();
        storage_map.get(&record_id).unwrap().clone()
    };

    // Verify integrity
    assert!(storage.verify_integrity(&encrypted).is_ok());

    // Tamper with ciphertext
    let mut tampered = encrypted.clone();
    if !tampered.ciphertext.is_empty() {
        tampered.ciphertext[0] ^= 0xFF;
    }

    // Should detect tampering
    let result = storage.verify_integrity(&tampered);
    assert!(matches!(result, Err(HipaaError::IntegrityCheckFailed)));
}
