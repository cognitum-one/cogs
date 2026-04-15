//! Capability token management

use alloc::collections::BTreeMap;
use agentvm_types::{Capability, CapabilityId, CapsuleId};

/// Capability table for managing tokens
pub struct CapabilityTable {
    /// Map of capability ID to capability
    capabilities: BTreeMap<CapabilityId, CapabilityEntry>,
    /// Index by capsule ID
    by_capsule: BTreeMap<CapsuleId, alloc::vec::Vec<CapabilityId>>,
}

/// Entry in the capability table
struct CapabilityEntry {
    capability: Capability,
    capsule_id: CapsuleId,
}

impl CapabilityTable {
    /// Create a new empty table
    pub fn new() -> Self {
        Self {
            capabilities: BTreeMap::new(),
            by_capsule: BTreeMap::new(),
        }
    }

    /// Insert a capability for a capsule
    pub fn insert(&mut self, capsule_id: CapsuleId, capability: Capability) {
        let cap_id = capability.id;

        self.capabilities.insert(
            cap_id,
            CapabilityEntry {
                capability,
                capsule_id,
            },
        );

        self.by_capsule
            .entry(capsule_id)
            .or_insert_with(alloc::vec::Vec::new)
            .push(cap_id);
    }

    /// Get a capability by ID
    pub fn get(&self, cap_id: CapabilityId) -> Option<&Capability> {
        self.capabilities.get(&cap_id).map(|e| &e.capability)
    }

    /// Get a mutable reference to a capability
    pub fn get_mut(&mut self, cap_id: CapabilityId) -> Option<&mut Capability> {
        self.capabilities.get_mut(&cap_id).map(|e| &mut e.capability)
    }

    /// Remove a capability
    pub fn remove(&mut self, cap_id: CapabilityId) -> Option<Capability> {
        if let Some(entry) = self.capabilities.remove(&cap_id) {
            // Remove from capsule index
            if let Some(caps) = self.by_capsule.get_mut(&entry.capsule_id) {
                caps.retain(|id| *id != cap_id);
            }
            Some(entry.capability)
        } else {
            None
        }
    }

    /// Get all capabilities for a capsule
    pub fn get_by_capsule(&self, capsule_id: CapsuleId) -> alloc::vec::Vec<&Capability> {
        self.by_capsule
            .get(&capsule_id)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.get(*id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Revoke a capability and all its descendants
    pub fn revoke(&mut self, cap_id: CapabilityId) -> bool {
        if let Some(cap) = self.get_mut(cap_id) {
            cap.revoked = true;

            // Find and revoke all descendants
            let descendants: alloc::vec::Vec<_> = self
                .capabilities
                .iter()
                .filter(|(_, e)| e.capability.parent == Some(cap_id))
                .map(|(id, _)| *id)
                .collect();

            for desc_id in descendants {
                self.revoke(desc_id);
            }

            true
        } else {
            false
        }
    }

    /// Revoke all capabilities for a capsule
    pub fn revoke_all(&mut self, capsule_id: CapsuleId) {
        if let Some(cap_ids) = self.by_capsule.get(&capsule_id).cloned() {
            for cap_id in cap_ids {
                self.revoke(cap_id);
            }
        }
    }

    /// Count capabilities
    pub fn len(&self) -> usize {
        self.capabilities.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.capabilities.is_empty()
    }

    /// Clean up expired and revoked capabilities
    pub fn cleanup(&mut self, current_time: u64) {
        let to_remove: alloc::vec::Vec<_> = self
            .capabilities
            .iter()
            .filter(|(_, e)| e.capability.is_expired(current_time) || e.capability.is_revoked())
            .map(|(id, _)| *id)
            .collect();

        for cap_id in to_remove {
            self.remove(cap_id);
        }
    }
}

impl Default for CapabilityTable {
    fn default() -> Self {
        Self::new()
    }
}
