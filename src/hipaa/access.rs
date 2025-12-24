//! Minimum necessary access control for HIPAA compliance
//!
//! Implements role-based access control ensuring users only access
//! the minimum PHI necessary for their role.

use super::{HipaaError, PhiView, Result, User};
use serde::{Deserialize, Serialize};

/// RBAC roles for PHI access
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// Full access to all PHI
    Clinician,
    /// Access to analysis results only
    LabTechnician,
    /// Full administrative access
    Administrator,
    /// Audit log access only
    Auditor,
    /// Read-only access
    ReadOnly,
}

impl Role {
    /// Get allowed PHI fields for this role
    pub fn allowed_fields(&self) -> Vec<PhiField> {
        match self {
            Role::Clinician | Role::Administrator => vec![
                PhiField::PatientId,
                PhiField::DnaSequence,
                PhiField::AnalysisResults,
            ],
            Role::LabTechnician => vec![PhiField::AnalysisResults],
            Role::Auditor => vec![],
            Role::ReadOnly => vec![PhiField::AnalysisResults],
        }
    }

    /// Check if role can access specific field
    pub fn can_access_field(&self, field: PhiField) -> bool {
        self.allowed_fields().contains(&field)
    }
}

/// PHI fields that can be filtered
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhiField {
    PatientId,
    DnaSequence,
    AnalysisResults,
}

/// Access control decision
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessDecision {
    Allow,
    Deny { reason: String },
}

impl AccessDecision {
    pub fn is_allow(&self) -> bool {
        matches!(self, AccessDecision::Allow)
    }

    pub fn is_deny(&self) -> bool {
        matches!(self, AccessDecision::Deny { .. })
    }
}

/// Minimum necessary access controller
pub struct MinimumNecessaryAccess;

impl MinimumNecessaryAccess {
    /// Check if user can access specific PHI field
    pub fn can_access(&self, user: &User, field: PhiField) -> AccessDecision {
        // Parse user roles
        let has_access = user.roles.iter().any(|role_str| {
            match role_str.as_str() {
                "clinician" => Role::Clinician.can_access_field(field.clone()),
                "lab_tech" => Role::LabTechnician.can_access_field(field.clone()),
                "admin" | "administrator" => Role::Administrator.can_access_field(field.clone()),
                "auditor" => Role::Auditor.can_access_field(field.clone()),
                "readonly" => Role::ReadOnly.can_access_field(field.clone()),
                _ => false,
            }
        });

        if has_access {
            AccessDecision::Allow
        } else {
            AccessDecision::Deny {
                reason: format!(
                    "User {} does not have permission to access {:?}",
                    user.id, field
                ),
            }
        }
    }

    /// Filter PHI view based on user's minimum necessary access
    pub fn filter_phi(&self, view: &mut PhiView, user: &User) -> Result<()> {
        // Check patient ID access
        if let Some(_) = &view.patient_id {
            if !self
                .can_access(user, PhiField::PatientId)
                .is_allow()
            {
                view.patient_id = None;
            }
        }

        // Check DNA sequence access
        if let Some(_) = &view.dna_sequence {
            if !self
                .can_access(user, PhiField::DnaSequence)
                .is_allow()
            {
                view.dna_sequence = None;
            }
        }

        // Check analysis results access
        if let Some(_) = &view.analysis_results {
            if !self
                .can_access(user, PhiField::AnalysisResults)
                .is_allow()
            {
                view.analysis_results = None;
            }
        }

        Ok(())
    }
}

/// Access control trait
pub trait AccessControl: Send + Sync {
    /// Check if user can perform action
    fn check_access(&self, user: &User, field: PhiField) -> AccessDecision;
}

impl AccessControl for MinimumNecessaryAccess {
    fn check_access(&self, user: &User, field: PhiField) -> AccessDecision {
        self.can_access(user, field)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hipaa::{AnalysisResult, UserId};

    #[test]
    fn test_clinician_full_access() {
        let role = Role::Clinician;
        assert!(role.can_access_field(PhiField::PatientId));
        assert!(role.can_access_field(PhiField::DnaSequence));
        assert!(role.can_access_field(PhiField::AnalysisResults));
    }

    #[test]
    fn test_lab_tech_limited_access() {
        let role = Role::LabTechnician;
        assert!(!role.can_access_field(PhiField::PatientId));
        assert!(!role.can_access_field(PhiField::DnaSequence));
        assert!(role.can_access_field(PhiField::AnalysisResults));
    }

    #[test]
    fn test_access_decision_for_lab_tech() {
        let access_control = MinimumNecessaryAccess;
        let user = User::new("tech_001", vec!["lab_tech".to_string()]);

        // Lab tech can access analysis results
        let decision = access_control.can_access(&user, PhiField::AnalysisResults);
        assert!(decision.is_allow());

        // Lab tech cannot access DNA sequence
        let decision = access_control.can_access(&user, PhiField::DnaSequence);
        assert!(decision.is_deny());
    }

    #[test]
    fn test_filter_phi_for_lab_tech() {
        let access_control = MinimumNecessaryAccess;
        let user = User::new("tech_001", vec!["lab_tech".to_string()]);

        let mut view = PhiView {
            patient_id: Some("P12345".to_string()),
            dna_sequence: Some("ATCG".to_string()),
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
    fn test_filter_phi_for_clinician() {
        let access_control = MinimumNecessaryAccess;
        let user = User::new("doc_001", vec!["clinician".to_string()]);

        let mut view = PhiView {
            patient_id: Some("P12345".to_string()),
            dna_sequence: Some("ATCG".to_string()),
            analysis_results: Some(vec![]),
        };

        access_control.filter_phi(&mut view, &user).unwrap();

        // All fields should remain
        assert!(view.patient_id.is_some());
        assert!(view.dna_sequence.is_some());
        assert!(view.analysis_results.is_some());
    }
}
