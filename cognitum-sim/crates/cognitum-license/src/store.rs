//! License storage and persistence

use crate::{License, LicenseError};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Trait for license storage
pub trait LicenseStore: Send + Sync {
    /// Save a license
    fn save(&self, license: &License) -> Result<(), LicenseError>;

    /// Get a license by key
    fn get(&self, key: &str) -> Result<License, LicenseError>;

    /// Delete a license
    fn delete(&self, key: &str) -> Result<(), LicenseError>;

    /// List all license keys
    fn list_keys(&self) -> Result<Vec<String>, LicenseError>;

    /// Check if license exists
    fn exists(&self, key: &str) -> bool {
        self.get(key).is_ok()
    }
}

/// In-memory license store (for testing and development)
pub struct InMemoryStore {
    licenses: Arc<RwLock<HashMap<String, License>>>,
}

impl InMemoryStore {
    /// Create a new in-memory store
    pub fn new() -> Self {
        Self {
            licenses: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Clear all licenses
    pub fn clear(&self) {
        self.licenses.write().clear();
    }

    /// Get number of stored licenses
    pub fn len(&self) -> usize {
        self.licenses.read().len()
    }

    /// Check if store is empty
    pub fn is_empty(&self) -> bool {
        self.licenses.read().is_empty()
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl LicenseStore for InMemoryStore {
    fn save(&self, license: &License) -> Result<(), LicenseError> {
        self.licenses
            .write()
            .insert(license.key.clone(), license.clone());
        Ok(())
    }

    fn get(&self, key: &str) -> Result<License, LicenseError> {
        self.licenses
            .read()
            .get(key)
            .cloned()
            .ok_or_else(|| LicenseError::NotFound {
                key: key.to_string(),
            })
    }

    fn delete(&self, key: &str) -> Result<(), LicenseError> {
        self.licenses
            .write()
            .remove(key)
            .ok_or_else(|| LicenseError::NotFound {
                key: key.to_string(),
            })?;
        Ok(())
    }

    fn list_keys(&self) -> Result<Vec<String>, LicenseError> {
        Ok(self.licenses.read().keys().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::license::LicenseTier;
    use chrono::Utc;

    fn create_test_license(key: &str) -> License {
        License {
            key: key.to_string(),
            tier: LicenseTier::Developer,
            organization: "Test".to_string(),
            email: "test@example.com".to_string(),
            max_tiles: 256,
            max_simulations_per_month: None,
            features: vec![],
            valid_until: Utc::now() + chrono::Duration::days(365),
            issued_at: Utc::now(),
            signature: vec![0; 64],
            metadata: Default::default(),
        }
    }

    #[test]
    fn test_save_and_get() {
        let store = InMemoryStore::new();
        let license = create_test_license("test_key_1");

        store.save(&license).unwrap();
        let retrieved = store.get("test_key_1").unwrap();

        assert_eq!(retrieved.key, license.key);
        assert_eq!(retrieved.organization, license.organization);
    }

    #[test]
    fn test_get_nonexistent() {
        let store = InMemoryStore::new();
        let result = store.get("nonexistent");

        assert!(matches!(result, Err(LicenseError::NotFound { .. })));
    }

    #[test]
    fn test_delete() {
        let store = InMemoryStore::new();
        let license = create_test_license("test_key_2");

        store.save(&license).unwrap();
        assert!(store.exists("test_key_2"));

        store.delete("test_key_2").unwrap();
        assert!(!store.exists("test_key_2"));
    }

    #[test]
    fn test_list_keys() {
        let store = InMemoryStore::new();

        store.save(&create_test_license("key1")).unwrap();
        store.save(&create_test_license("key2")).unwrap();
        store.save(&create_test_license("key3")).unwrap();

        let keys = store.list_keys().unwrap();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"key1".to_string()));
        assert!(keys.contains(&"key2".to_string()));
        assert!(keys.contains(&"key3".to_string()));
    }

    #[test]
    fn test_clear() {
        let store = InMemoryStore::new();

        store.save(&create_test_license("key1")).unwrap();
        store.save(&create_test_license("key2")).unwrap();

        assert_eq!(store.len(), 2);

        store.clear();

        assert_eq!(store.len(), 0);
        assert!(store.is_empty());
    }
}
