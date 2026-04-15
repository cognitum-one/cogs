//! Acceptance tests for minimum necessary access control
//!
//! Validates HIPAA §164.514(d) - Minimum necessary standard

use cognitum::hipaa::*;

#[tokio::test]
async fn lab_technician_should_only_access_analysis_results() {
    // Given: PHI record in storage
    let storage = HipaaCompliantStorage::new(StorageConfig::default());
    let phi_record = PhiRecord {
        patient_id: "P98765".to_string(),
        dna_sequence: "ATCGATCGATCGATCG".to_string(),
        analysis_results: vec![
            AnalysisResult {
                test_name: "Gene Expression Analysis".to_string(),
                result_value: "Within normal range".to_string(),
                timestamp: chrono::Utc::now(),
            },
            AnalysisResult {
                test_name: "Variant Detection".to_string(),
                result_value: "No pathogenic variants detected".to_string(),
                timestamp: chrono::Utc::now(),
            },
        ],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let record_id = storage.store(phi_record).await.unwrap();

    // When: Lab technician accesses the record
    let lab_tech = User::new("tech_lab_001", vec!["lab_tech".to_string()]);
    let view = storage.get_for_user(&record_id, &lab_tech).await.unwrap();

    // Then: Only analysis results should be visible
    assert!(view.analysis_results.is_some(), "Lab tech should see analysis results");
    assert!(view.dna_sequence.is_none(), "Lab tech should NOT see raw DNA sequence");
    assert!(view.patient_id.is_none(), "Lab tech should NOT see patient ID");

    // And: Analysis results should contain expected data
    let results = view.analysis_results.unwrap();
    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn clinician_should_access_full_phi_record() {
    // Given: PHI record in storage
    let storage = HipaaCompliantStorage::new(StorageConfig::default());
    let phi_record = PhiRecord {
        patient_id: "P54321".to_string(),
        dna_sequence: "GCTAGCTAGCTAGCTA".to_string(),
        analysis_results: vec![AnalysisResult {
            test_name: "Diagnostic Test".to_string(),
            result_value: "Positive".to_string(),
            timestamp: chrono::Utc::now(),
        }],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let record_id = storage.store(phi_record.clone()).await.unwrap();

    // When: Clinician accesses the record
    let clinician = User::new("dr_smith", vec!["clinician".to_string()]);
    let view = storage.get_for_user(&record_id, &clinician).await.unwrap();

    // Then: All PHI fields should be visible
    assert!(view.patient_id.is_some(), "Clinician should see patient ID");
    assert!(view.dna_sequence.is_some(), "Clinician should see DNA sequence");
    assert!(view.analysis_results.is_some(), "Clinician should see analysis results");

    assert_eq!(view.patient_id.unwrap(), phi_record.patient_id);
    assert_eq!(view.dna_sequence.unwrap(), phi_record.dna_sequence);
}

#[tokio::test]
async fn unauthorized_user_should_be_denied_access() {
    // Given: PHI record in storage
    let storage = HipaaCompliantStorage::new(StorageConfig::default());
    let phi_record = PhiRecord {
        patient_id: "P11111".to_string(),
        dna_sequence: "ATATATATAT".to_string(),
        analysis_results: vec![],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let record_id = storage.store(phi_record).await.unwrap();

    // When: Unauthorized user attempts access
    let unauthorized = User::new("guest_user", vec!["guest".to_string()]);
    let result = storage.get_for_user(&record_id, &unauthorized).await;

    // Then: Access should be denied
    assert!(
        matches!(result, Err(HipaaError::InsufficientPermissions)),
        "Unauthorized user should be denied PHI access"
    );
}

#[tokio::test]
async fn administrator_should_have_full_access() {
    // Given: PHI record in storage
    let storage = HipaaCompliantStorage::new(StorageConfig::default());
    let phi_record = PhiRecord {
        patient_id: "P99999".to_string(),
        dna_sequence: "CGCGCGCGCG".to_string(),
        analysis_results: vec![],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    let record_id = storage.store(phi_record.clone()).await.unwrap();

    // When: Administrator accesses the record
    let admin = User::new("admin_001", vec!["admin".to_string()]);
    let view = storage.get_for_user(&record_id, &admin).await.unwrap();

    // Then: All fields should be accessible
    assert!(view.patient_id.is_some());
    assert!(view.dna_sequence.is_some());
    assert!(view.analysis_results.is_some());
}

#[test]
fn access_control_should_enforce_role_permissions() {
    // Given: Minimum necessary access controller
    let access_control = MinimumNecessaryAccess;

    // When/Then: Lab tech permissions
    let lab_tech = User::new("tech_001", vec!["lab_tech".to_string()]);
    assert!(access_control
        .can_access(&lab_tech, PhiField::AnalysisResults)
        .is_allow());
    assert!(access_control
        .can_access(&lab_tech, PhiField::DnaSequence)
        .is_deny());
    assert!(access_control
        .can_access(&lab_tech, PhiField::PatientId)
        .is_deny());

    // When/Then: Clinician permissions
    let clinician = User::new("doc_001", vec!["clinician".to_string()]);
    assert!(access_control
        .can_access(&clinician, PhiField::AnalysisResults)
        .is_allow());
    assert!(access_control
        .can_access(&clinician, PhiField::DnaSequence)
        .is_allow());
    assert!(access_control
        .can_access(&clinician, PhiField::PatientId)
        .is_allow());

    // When/Then: Auditor permissions (no PHI access)
    let auditor = User::new("audit_001", vec!["auditor".to_string()]);
    assert!(access_control
        .can_access(&auditor, PhiField::AnalysisResults)
        .is_deny());
    assert!(access_control
        .can_access(&auditor, PhiField::DnaSequence)
        .is_deny());
    assert!(access_control
        .can_access(&auditor, PhiField::PatientId)
        .is_deny());
}

#[test]
fn phi_filtering_should_respect_minimum_necessary() {
    // Given: Access controller and PHI view
    let access_control = MinimumNecessaryAccess;

    // When: Filtering for lab technician
    let lab_tech = User::new("tech_001", vec!["lab_tech".to_string()]);
    let mut view = PhiView {
        patient_id: Some("P12345".to_string()),
        dna_sequence: Some("ATCGATCG".to_string()),
        analysis_results: Some(vec![AnalysisResult {
            test_name: "Test".to_string(),
            result_value: "Result".to_string(),
            timestamp: chrono::Utc::now(),
        }]),
    };

    access_control.filter_phi(&mut view, &lab_tech).unwrap();

    // Then: Only necessary fields remain
    assert!(view.patient_id.is_none());
    assert!(view.dna_sequence.is_none());
    assert!(view.analysis_results.is_some());
}

#[test]
fn readonly_user_should_have_limited_access() {
    // Given: Read-only user
    let readonly = User::new("readonly_001", vec!["readonly".to_string()]);
    let access_control = MinimumNecessaryAccess;

    // When/Then: Can view analysis results only
    assert!(access_control
        .can_access(&readonly, PhiField::AnalysisResults)
        .is_allow());
    assert!(access_control
        .can_access(&readonly, PhiField::DnaSequence)
        .is_deny());
    assert!(access_control
        .can_access(&readonly, PhiField::PatientId)
        .is_deny());
}
