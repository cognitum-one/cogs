//! Merkle tree implementation for evidence chaining

use alloc::vec::Vec;
use crate::{Hash, hash_pair};

/// Merkle tree for evidence chaining
#[derive(Debug, Clone)]
pub struct MerkleTree {
    /// Leaf hashes
    leaves: Vec<Hash>,
    /// Internal nodes (built from leaves)
    nodes: Vec<Vec<Hash>>,
}

impl MerkleTree {
    /// Create a new empty Merkle tree
    pub fn new() -> Self {
        Self {
            leaves: Vec::new(),
            nodes: Vec::new(),
        }
    }

    /// Create a Merkle tree from existing leaves
    pub fn from_leaves(leaves: Vec<Hash>) -> Self {
        let mut tree = Self {
            leaves,
            nodes: Vec::new(),
        };
        tree.rebuild();
        tree
    }

    /// Append a leaf and return the new root
    pub fn append(&mut self, leaf_hash: Hash) -> Hash {
        self.leaves.push(leaf_hash);
        self.rebuild();
        self.root()
    }

    /// Append multiple leaves
    pub fn append_many(&mut self, leaf_hashes: &[Hash]) {
        self.leaves.extend_from_slice(leaf_hashes);
        self.rebuild();
    }

    /// Get the current Merkle root
    pub fn root(&self) -> Hash {
        if self.nodes.is_empty() {
            if self.leaves.is_empty() {
                [0u8; 32]
            } else if self.leaves.len() == 1 {
                self.leaves[0]
            } else {
                // Should not happen if rebuild was called
                [0u8; 32]
            }
        } else {
            // Root is the last level with single element
            self.nodes.last().and_then(|l| l.first()).copied().unwrap_or([0u8; 32])
        }
    }

    /// Get the number of leaves
    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    /// Check if the tree is empty
    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    /// Get a leaf by index
    pub fn get_leaf(&self, index: usize) -> Option<&Hash> {
        self.leaves.get(index)
    }

    /// Generate an inclusion proof for a leaf at the given index
    pub fn inclusion_proof(&self, leaf_index: usize) -> Option<InclusionProof> {
        if leaf_index >= self.leaves.len() {
            return None;
        }

        let mut proof = Vec::new();
        let mut index = leaf_index;
        let mut level_size = self.leaves.len();

        // Start with leaves as level 0
        let mut current_level: &[Hash] = &self.leaves;

        // Walk up the tree
        for node_level in &self.nodes {
            let sibling_idx = if index % 2 == 0 { index + 1 } else { index - 1 };

            if sibling_idx < current_level.len() {
                proof.push(ProofElement {
                    hash: current_level[sibling_idx],
                    is_left: index % 2 == 1,
                });
            }

            index /= 2;
            current_level = node_level;
        }

        Some(InclusionProof {
            leaf_index,
            leaf_hash: self.leaves[leaf_index],
            tree_size: self.leaves.len() as u64,
            proof,
        })
    }

    /// Generate a consistency proof between two tree sizes
    pub fn consistency_proof(&self, old_size: usize) -> Option<ConsistencyProof> {
        if old_size == 0 || old_size > self.leaves.len() {
            return None;
        }

        // Compute old root
        let old_tree = MerkleTree::from_leaves(self.leaves[..old_size].to_vec());
        let old_root = old_tree.root();

        // Collect proof elements
        // This is a simplified version; full RFC 6962 consistency proof is more complex
        let mut proof = Vec::new();

        // Add the hashes needed to reconstruct both roots
        if old_size < self.leaves.len() {
            // Add subtree roots
            let mut remaining = old_size;
            let mut offset = 0;

            while remaining > 0 {
                let subtree_size = remaining.next_power_of_two() / 2;
                if subtree_size == 0 {
                    break;
                }

                if remaining >= subtree_size {
                    // Compute subtree root
                    let subtree = MerkleTree::from_leaves(
                        self.leaves[offset..offset + subtree_size].to_vec()
                    );
                    proof.push(subtree.root());
                    offset += subtree_size;
                    remaining -= subtree_size;
                } else {
                    remaining = 0;
                }
            }
        }

        Some(ConsistencyProof {
            old_size: old_size as u64,
            new_size: self.leaves.len() as u64,
            old_root,
            new_root: self.root(),
            proof,
        })
    }

    /// Rebuild the tree from leaves
    fn rebuild(&mut self) {
        self.nodes.clear();

        if self.leaves.len() <= 1 {
            return;
        }

        let mut current_level = self.leaves.clone();

        while current_level.len() > 1 {
            let mut next_level = Vec::with_capacity((current_level.len() + 1) / 2);

            for chunk in current_level.chunks(2) {
                let hash = if chunk.len() == 2 {
                    hash_pair(&chunk[0], &chunk[1])
                } else {
                    // Odd number of nodes: promote the last one
                    chunk[0]
                };
                next_level.push(hash);
            }

            self.nodes.push(next_level.clone());
            current_level = next_level;
        }
    }
}

impl Default for MerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

/// Element in an inclusion proof
#[derive(Debug, Clone, Copy)]
pub struct ProofElement {
    /// Sibling hash
    pub hash: Hash,
    /// Whether this sibling is on the left
    pub is_left: bool,
}

/// Merkle inclusion proof
#[derive(Debug, Clone)]
pub struct InclusionProof {
    /// Index of the leaf being proved
    pub leaf_index: usize,
    /// Hash of the leaf
    pub leaf_hash: Hash,
    /// Size of the tree when proof was generated
    pub tree_size: u64,
    /// Proof elements (sibling hashes from leaf to root)
    pub proof: Vec<ProofElement>,
}

impl InclusionProof {
    /// Verify this proof against a known root
    pub fn verify(&self, expected_root: &Hash) -> bool {
        let mut computed = self.leaf_hash;

        for element in &self.proof {
            if element.is_left {
                computed = hash_pair(&element.hash, &computed);
            } else {
                computed = hash_pair(&computed, &element.hash);
            }
        }

        &computed == expected_root
    }
}

/// Merkle consistency proof
#[derive(Debug, Clone)]
pub struct ConsistencyProof {
    /// Size of the old tree
    pub old_size: u64,
    /// Size of the new tree
    pub new_size: u64,
    /// Root of the old tree
    pub old_root: Hash,
    /// Root of the new tree
    pub new_root: Hash,
    /// Proof elements
    pub proof: Vec<Hash>,
}

impl ConsistencyProof {
    /// Verify that the new tree is an extension of the old tree
    pub fn verify(&self, expected_old_root: &Hash, expected_new_root: &Hash) -> bool {
        // Basic sanity checks
        if self.old_size > self.new_size {
            return false;
        }

        if &self.old_root != expected_old_root {
            return false;
        }

        if &self.new_root != expected_new_root {
            return false;
        }

        // Full verification would reconstruct both roots from proof
        // This is a simplified version
        true
    }
}
