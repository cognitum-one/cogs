//! BAA (Business Associate Agreement) workflow for HIPAA compliance
//!
//! Enterprise customers must sign BAA before accessing PHI features.

use super::{Customer, CustomerId, HipaaError, Result, Tier};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// BAA signature record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaaSignature {
    pub customer_id: CustomerId,
    pub signed_at: DateTime<Utc>,
    pub signed_by: String,
    pub version: String,
    pub document_hash: String,
}

/// Onboarding error
#[derive(Debug, thiserror::Error)]
pub enum OnboardingError {
    #[error("BAA signature required for PHI features")]
    BaaRequired,

    #[error("Invalid tier for PHI features: {0:?}")]
    InvalidTier(Tier),

    #[error("Customer not found: {0}")]
    CustomerNotFound(CustomerId),

    #[error("BAA already signed for customer: {0}")]
    BaaAlreadySigned(CustomerId),
}

impl From<OnboardingError> for HipaaError {
    fn from(err: OnboardingError) -> Self {
        match err {
            OnboardingError::BaaRequired => HipaaError::BaaRequired,
            OnboardingError::InvalidTier(tier) => {
                HipaaError::AccessDenied(format!("Invalid tier for PHI: {:?}", tier))
            }
            OnboardingError::CustomerNotFound(id) => {
                HipaaError::AccessDenied(format!("Customer not found: {}", id))
            }
            OnboardingError::BaaAlreadySigned(id) => {
                HipaaError::AccessDenied(format!("BAA already signed: {}", id))
            }
        }
    }
}

/// BAA workflow trait
pub trait BaaWorkflow: Send + Sync {
    /// Check if customer has signed BAA
    fn has_signed_baa(&self, customer_id: &CustomerId) -> Result<bool>;

    /// Sign BAA for customer
    fn sign_baa(&self, signature: BaaSignature) -> Result<()>;

    /// Verify BAA is valid and current
    fn verify_baa(&self, customer_id: &CustomerId) -> Result<bool>;
}

/// Customer onboarding service
pub struct CustomerOnboarding {
    baa_signatures: Arc<RwLock<HashMap<CustomerId, BaaSignature>>>,
    customers: Arc<RwLock<HashMap<CustomerId, Customer>>>,
}

impl CustomerOnboarding {
    /// Create new customer onboarding service
    pub fn new() -> Self {
        Self {
            baa_signatures: Arc::new(RwLock::new(HashMap::new())),
            customers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a customer
    pub fn register_customer(&self, customer: Customer) -> Result<()> {
        self.customers
            .write()
            .unwrap()
            .insert(customer.id.clone(), customer);
        Ok(())
    }

    /// Get customer
    pub fn get_customer(&self, customer_id: &CustomerId) -> Result<Customer> {
        self.customers
            .read()
            .unwrap()
            .get(customer_id)
            .cloned()
            .ok_or_else(|| {
                HipaaError::AccessDenied(format!("Customer not found: {}", customer_id))
            })
    }

    /// Enable PHI features for customer
    pub async fn enable_phi_features(&self, customer: &Customer) -> Result<()> {
        // Only enterprise tier can have PHI features
        if customer.tier != Tier::Enterprise {
            return Err(OnboardingError::InvalidTier(customer.tier.clone()).into());
        }

        // BAA must be signed
        if !customer.baa_signed {
            return Err(OnboardingError::BaaRequired.into());
        }

        // Verify BAA is on file
        let has_baa = self.has_signed_baa(&customer.id)?;
        if !has_baa {
            return Err(OnboardingError::BaaRequired.into());
        }

        // Enable PHI features (implementation would update customer record)
        Ok(())
    }

    /// Sign BAA for customer
    pub fn sign_baa_for_customer(
        &self,
        customer_id: &CustomerId,
        signed_by: String,
        document_hash: String,
    ) -> Result<BaaSignature> {
        let signatures = self.baa_signatures.read().unwrap();
        if signatures.contains_key(customer_id) {
            drop(signatures);
            return Err(OnboardingError::BaaAlreadySigned(customer_id.clone()).into());
        }
        drop(signatures);

        let signature = BaaSignature {
            customer_id: customer_id.clone(),
            signed_at: Utc::now(),
            signed_by,
            version: "1.0".to_string(),
            document_hash,
        };

        self.sign_baa(signature.clone())?;

        // Update customer record
        let mut customers = self.customers.write().unwrap();
        if let Some(customer) = customers.get_mut(customer_id) {
            customer.baa_signed = true;
            customer.baa_signed_at = Some(signature.signed_at);
        }

        Ok(signature)
    }
}

impl BaaWorkflow for CustomerOnboarding {
    fn has_signed_baa(&self, customer_id: &CustomerId) -> Result<bool> {
        let signatures = self.baa_signatures.read().unwrap();
        Ok(signatures.contains_key(customer_id))
    }

    fn sign_baa(&self, signature: BaaSignature) -> Result<()> {
        self.baa_signatures
            .write()
            .unwrap()
            .insert(signature.customer_id.clone(), signature);
        Ok(())
    }

    fn verify_baa(&self, customer_id: &CustomerId) -> Result<bool> {
        let signatures = self.baa_signatures.read().unwrap();
        Ok(signatures.contains_key(customer_id))
    }
}

impl Default for CustomerOnboarding {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_customer(id: &str, tier: Tier, baa_signed: bool) -> Customer {
        Customer {
            id: CustomerId::new(id),
            tier,
            baa_signed,
            baa_signed_at: if baa_signed {
                Some(Utc::now())
            } else {
                None
            },
        }
    }

    #[tokio::test]
    async fn test_enterprise_without_baa_denied() {
        let onboarding = CustomerOnboarding::new();
        let customer = create_test_customer("cust_123", Tier::Enterprise, false);

        let result = onboarding.enable_phi_features(&customer).await;
        assert!(matches!(result, Err(HipaaError::BaaRequired)));
    }

    #[tokio::test]
    async fn test_enterprise_with_baa_allowed() {
        let onboarding = CustomerOnboarding::new();
        let customer_id = CustomerId::new("cust_123");
        let mut customer = create_test_customer("cust_123", Tier::Enterprise, false);

        // Register customer
        onboarding.register_customer(customer.clone()).unwrap();

        // Sign BAA
        let signature = onboarding
            .sign_baa_for_customer(&customer_id, "John Doe".to_string(), "hash123".to_string())
            .unwrap();

        assert_eq!(signature.customer_id, customer_id);

        // Get updated customer
        let updated_customer = onboarding.get_customer(&customer_id).unwrap();
        assert!(updated_customer.baa_signed);

        // Enable PHI features should now succeed
        let result = onboarding.enable_phi_features(&updated_customer).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_free_tier_cannot_access_phi() {
        let onboarding = CustomerOnboarding::new();
        let customer = create_test_customer("cust_123", Tier::Free, false);

        let result = onboarding.enable_phi_features(&customer).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_baa_signature_creation() {
        let onboarding = CustomerOnboarding::new();
        let customer_id = CustomerId::new("cust_123");
        let customer = create_test_customer("cust_123", Tier::Enterprise, false);

        onboarding.register_customer(customer).unwrap();

        let signature = onboarding
            .sign_baa_for_customer(&customer_id, "Jane Smith".to_string(), "hash456".to_string())
            .unwrap();

        assert_eq!(signature.signed_by, "Jane Smith");
        assert_eq!(signature.document_hash, "hash456");
        assert!(onboarding.has_signed_baa(&customer_id).unwrap());
    }

    #[test]
    fn test_duplicate_baa_signature_denied() {
        let onboarding = CustomerOnboarding::new();
        let customer_id = CustomerId::new("cust_123");
        let customer = create_test_customer("cust_123", Tier::Enterprise, false);

        onboarding.register_customer(customer).unwrap();

        // First signature succeeds
        let _ = onboarding
            .sign_baa_for_customer(&customer_id, "Person 1".to_string(), "hash1".to_string())
            .unwrap();

        // Second signature fails
        let result = onboarding.sign_baa_for_customer(
            &customer_id,
            "Person 2".to_string(),
            "hash2".to_string(),
        );

        assert!(result.is_err());
    }
}
