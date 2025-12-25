//! Unit tests for minimum necessary access control

use cognitum::hipaa::*;

#[test]
fn clinician_role_has_full_access() {
    let role = Role::Clinician;
    assert!(role.can_access_field(PhiField::PatientId));
    assert!(role.can_access_field(PhiField::DnaSequence));
    assert!(role.can_access_field(PhiField::AnalysisResults));
}

#[test]
fn lab_tech_role_has_limited_access() {
    let role = Role::LabTechnician;
    assert!(!role.can_access_field(PhiField::PatientId));
    assert!(!role.can_access_field(PhiField::DnaSequence));
    assert!(role.can_access_field(PhiField::AnalysisResults));
}

#[test]
fn administrator_role_has_full_access() {
    let role = Role::Administrator;
    assert!(role.can_access_field(PhiField::PatientId));
    assert!(role.can_access_field(PhiField::DnaSequence));
    assert!(role.can_access_field(PhiField::AnalysisResults));
}

#[test]
fn auditor_role_has_no_phi_access() {
    let role = Role::Auditor;
    assert!(!role.can_access_field(PhiField::PatientId));
    assert!(!role.can_access_field(PhiField::DnaSequence));
    assert!(!role.can_access_field(PhiField::AnalysisResults));
}

#[test]
fn readonly_role_has_results_only() {
    let role = Role::ReadOnly;
    assert!(!role.can_access_field(PhiField::PatientId));
    assert!(!role.can_access_field(PhiField::DnaSequence));
    assert!(role.can_access_field(PhiField::AnalysisResults));
}

#[test]
fn access_control_allows_authorized_user() {
    let access_control = MinimumNecessaryAccess;
    let user = User::new("user_123", vec!["clinician".to_string()]);

    let decision = access_control.can_access(&user, PhiField::DnaSequence);
    assert!(decision.is_allow());
}

#[test]
fn access_control_denies_unauthorized_user() {
    let access_control = MinimumNecessaryAccess;
    let user = User::new("user_123", vec!["lab_tech".to_string()]);

    let decision = access_control.can_access(&user, PhiField::DnaSequence);
    assert!(decision.is_deny());

    if let AccessDecision::Deny { reason } = decision {
        assert!(reason.contains("does not have permission"));
    }
}

#[test]
fn filter_phi_for_lab_tech() {
    let access_control = MinimumNecessaryAccess;
    let user = User::new("tech_001", vec!["lab_tech".to_string()]);

    let mut view = PhiView {
        patient_id: Some("P12345".to_string()),
        dna_sequence: Some("ATCGATCG".to_string()),
        analysis_results: Some(vec![AnalysisResult {
            test_name: "Test1".to_string(),
            result_value: "Normal".to_string(),
            timestamp: chrono::Utc::now(),
        }]),
    };

    access_control.filter_phi(&mut view, &user).unwrap();

    // Patient ID and DNA should be filtered out
    assert!(view.patient_id.is_none());
    assert!(view.dna_sequence.is_none());
    // Analysis results should remain
    assert!(view.analysis_results.is_some());
}

#[test]
fn filter_phi_for_clinician() {
    let access_control = MinimumNecessaryAccess;
    let user = User::new("doc_001", vec!["clinician".to_string()]);

    let mut view = PhiView {
        patient_id: Some("P12345".to_string()),
        dna_sequence: Some("ATCGATCG".to_string()),
        analysis_results: Some(vec![]),
    };

    access_control.filter_phi(&mut view, &user).unwrap();

    // All fields should remain
    assert!(view.patient_id.is_some());
    assert!(view.dna_sequence.is_some());
    assert!(view.analysis_results.is_some());
}

#[test]
fn filter_phi_for_auditor() {
    let access_control = MinimumNecessaryAccess;
    let user = User::new("audit_001", vec!["auditor".to_string()]);

    let mut view = PhiView {
        patient_id: Some("P12345".to_string()),
        dna_sequence: Some("ATCGATCG".to_string()),
        analysis_results: Some(vec![]),
    };

    access_control.filter_phi(&mut view, &user).unwrap();

    // All fields should be filtered
    assert!(view.patient_id.is_none());
    assert!(view.dna_sequence.is_none());
    assert!(view.analysis_results.is_none());
}

#[test]
fn multiple_roles_combine_permissions() {
    let access_control = MinimumNecessaryAccess;
    let user = User::new("user_001", vec!["lab_tech".to_string(), "readonly".to_string()]);

    // Both roles allow analysis results
    let decision = access_control.can_access(&user, PhiField::AnalysisResults);
    assert!(decision.is_allow());

    // Neither role allows DNA sequence
    let decision = access_control.can_access(&user, PhiField::DnaSequence);
    assert!(decision.is_deny());
}

#[test]
fn admin_override_grants_full_access() {
    let access_control = MinimumNecessaryAccess;
    let user = User::new("admin_001", vec!["admin".to_string()]);

    assert!(access_control
        .can_access(&user, PhiField::PatientId)
        .is_allow());
    assert!(access_control
        .can_access(&user, PhiField::DnaSequence)
        .is_allow());
    assert!(access_control
        .can_access(&user, PhiField::AnalysisResults)
        .is_allow());
}
