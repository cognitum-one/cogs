//! Mini-HNSW (Hierarchical Navigable Small World) index
//!
//! A simplified HNSW implementation optimized for embedded systems.
//! Supports up to 2000 vectors with M=8 connections per node.

use crate::error::ThermalBrainError;
use crate::types::{HnswStats, PatternVector, FEATURE_DIMS};
use heapless::Vec as HVec;

/// Maximum vectors in the index
const MAX_VECTORS: usize = 2000;

/// Maximum connections per node (M parameter)
const MAX_M: usize = 16;

/// Maximum layers
const MAX_LAYERS: usize = 4;

/// HNSW node - stores vector and connections
#[derive(Clone)]
struct HnswNode {
    /// Vector data
    vector: PatternVector,
    /// Connections at each layer
    connections: [HVec<u16, MAX_M>; MAX_LAYERS],
    /// Maximum layer this node exists on
    max_layer: u8,
    /// Whether this node is active (not deleted)
    active: bool,
}

impl Default for HnswNode {
    fn default() -> Self {
        Self {
            vector: [0i8; FEATURE_DIMS],
            connections: [
                HVec::new(),
                HVec::new(),
                HVec::new(),
                HVec::new(),
            ],
            max_layer: 0,
            active: false,
        }
    }
}

/// Mini-HNSW index for approximate nearest neighbor search
pub struct MiniHnsw {
    /// Nodes in the index
    nodes: HVec<HnswNode, MAX_VECTORS>,
    /// M parameter (connections per node)
    m: usize,
    /// ef_construction parameter
    ef_construction: usize,
    /// Entry point (node with highest layer)
    entry_point: Option<u16>,
    /// Current maximum layer
    max_layer: u8,
    /// Random state for layer selection
    random_state: u32,
    /// Total search hops (for statistics)
    total_search_hops: u64,
    /// Total searches (for statistics)
    total_searches: u64,
}

impl MiniHnsw {
    /// Create a new HNSW index
    ///
    /// # Arguments
    /// * `m` - Number of connections per node (typically 8-16)
    /// * `ef_construction` - Construction ef parameter (typically 50-200)
    pub fn new(m: usize, ef_construction: usize) -> Self {
        Self {
            nodes: HVec::new(),
            m: m.min(MAX_M),
            ef_construction,
            entry_point: None,
            max_layer: 0,
            random_state: 12345,
            total_search_hops: 0,
            total_searches: 0,
        }
    }

    /// Get number of vectors in the index
    pub fn len(&self) -> usize {
        self.nodes.iter().filter(|n| n.active).count()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Insert a vector into the index
    ///
    /// # Returns
    /// The ID of the inserted vector
    pub fn insert(&mut self, vector: PatternVector) -> Result<u32, ThermalBrainError> {
        if self.nodes.len() >= MAX_VECTORS {
            return Err(ThermalBrainError::PatternLimitReached);
        }

        let id = self.nodes.len() as u32;
        let layer = self.random_layer();

        // Create new node
        let mut node = HnswNode::default();
        node.vector = vector;
        node.max_layer = layer;
        node.active = true;

        self.nodes.push(node).map_err(|_| ThermalBrainError::OutOfMemory)?;

        // If this is the first node, set as entry point
        if self.entry_point.is_none() {
            self.entry_point = Some(id as u16);
            self.max_layer = layer;
            return Ok(id);
        }

        // Connect to existing graph
        let entry = self.entry_point.unwrap();

        // Search from top layer down to node's max layer + 1
        let mut current = entry;
        for l in (layer as usize + 1..=self.max_layer as usize).rev() {
            current = self.search_layer_single(&vector, current, l);
        }

        // At each layer from node's max_layer down to 0, find neighbors and connect
        for l in (0..=layer as usize).rev() {
            let neighbors = self.search_layer(&vector, current, l, self.ef_construction);
            self.connect_node(id as u16, &neighbors, l);

            // Use first neighbor as starting point for next layer
            if !neighbors.is_empty() {
                current = neighbors[0].0;
            }
        }

        // Update entry point if this node has higher layer
        if layer > self.max_layer {
            self.entry_point = Some(id as u16);
            self.max_layer = layer;
        }

        Ok(id)
    }

    /// Search for k nearest neighbors
    ///
    /// # Arguments
    /// * `query` - Query vector
    /// * `k` - Number of neighbors to return
    /// * `ef_search` - Search ef parameter (higher = more accurate but slower)
    ///
    /// # Returns
    /// Vector of (id, similarity) pairs
    pub fn search(&mut self, query: &PatternVector, k: usize, ef_search: usize) -> HVec<(u32, f32), 16> {
        self.total_searches += 1;

        if self.entry_point.is_none() {
            return HVec::new();
        }

        let mut current = self.entry_point.unwrap();

        // Descend through layers
        for l in (1..=self.max_layer as usize).rev() {
            current = self.search_layer_single(query, current, l);
        }

        // Search layer 0 with ef_search
        let candidates = self.search_layer(query, current, 0, ef_search);

        // Return top k
        let mut result = HVec::new();
        for (idx, sim) in candidates.iter().take(k) {
            let _ = result.push((*idx as u32, *sim));
        }
        result
    }

    /// Get index statistics
    pub fn stats(&self) -> HnswStats {
        let avg_hops = if self.total_searches > 0 {
            self.total_search_hops as f32 / self.total_searches as f32
        } else {
            0.0
        };

        HnswStats {
            num_vectors: self.len(),
            num_layers: self.max_layer as usize + 1,
            memory_bytes: self.estimate_memory(),
            avg_search_hops: avg_hops,
        }
    }

    /// Search single best candidate at given layer
    fn search_layer_single(&mut self, query: &PatternVector, start: u16, layer: usize) -> u16 {
        let mut current = start;
        let mut best_sim = self.similarity(query, &self.nodes[start as usize].vector);
        let mut changed = true;

        while changed {
            changed = false;
            self.total_search_hops += 1;

            let node = &self.nodes[current as usize];
            if layer < node.connections.len() {
                for &neighbor in node.connections[layer].iter() {
                    if !self.nodes[neighbor as usize].active {
                        continue;
                    }
                    let sim = self.similarity(query, &self.nodes[neighbor as usize].vector);
                    if sim > best_sim {
                        best_sim = sim;
                        current = neighbor;
                        changed = true;
                    }
                }
            }
        }

        current
    }

    /// Search layer for multiple candidates
    fn search_layer(
        &mut self,
        query: &PatternVector,
        start: u16,
        layer: usize,
        ef: usize,
    ) -> HVec<(u16, f32), 64> {
        let mut visited = HVec::<u16, 256>::new();
        let mut candidates = HVec::<(u16, f32), 64>::new();
        let mut results = HVec::<(u16, f32), 64>::new();

        let start_sim = self.similarity(query, &self.nodes[start as usize].vector);
        let _ = candidates.push((start, start_sim));
        let _ = results.push((start, start_sim));
        let _ = visited.push(start);

        while !candidates.is_empty() {
            // Get best candidate
            let mut best_idx = 0;
            for (i, &(_, sim)) in candidates.iter().enumerate() {
                if sim > candidates[best_idx].1 {
                    best_idx = i;
                }
            }
            let (current, current_sim) = candidates.remove(best_idx);

            // Get worst result
            let worst_result_sim = results.iter().map(|&(_, s)| s).fold(f32::MAX, f32::min);

            // If best candidate is worse than worst result, we're done
            if current_sim < worst_result_sim && results.len() >= ef {
                break;
            }

            self.total_search_hops += 1;

            // Explore neighbors
            let node = &self.nodes[current as usize];
            if layer < node.connections.len() {
                for &neighbor in node.connections[layer].iter() {
                    if visited.contains(&neighbor) {
                        continue;
                    }
                    let _ = visited.push(neighbor);

                    if !self.nodes[neighbor as usize].active {
                        continue;
                    }

                    let sim = self.similarity(query, &self.nodes[neighbor as usize].vector);

                    if sim > worst_result_sim || results.len() < ef {
                        let _ = candidates.push((neighbor, sim));
                        let _ = results.push((neighbor, sim));

                        // Keep results sorted and pruned
                        if results.len() > ef {
                            // Remove worst
                            let mut min_idx = 0;
                            for (i, &(_, s)) in results.iter().enumerate() {
                                if s < results[min_idx].1 {
                                    min_idx = i;
                                }
                            }
                            results.remove(min_idx);
                        }
                    }
                }
            }
        }

        // Sort results by similarity (descending)
        for i in 0..results.len() {
            for j in i + 1..results.len() {
                if results[j].1 > results[i].1 {
                    results.swap(i, j);
                }
            }
        }

        results
    }

    /// Connect a node to its neighbors at given layer
    fn connect_node(&mut self, node_id: u16, neighbors: &[(u16, f32)], layer: usize) {
        // Add connections from node to neighbors
        for &(neighbor_id, _) in neighbors.iter().take(self.m) {
            if layer < self.nodes[node_id as usize].connections.len() {
                let _ = self.nodes[node_id as usize].connections[layer].push(neighbor_id);
            }
        }

        // Add reverse connections
        for &(neighbor_id, _) in neighbors.iter().take(self.m) {
            if layer < self.nodes[neighbor_id as usize].connections.len() {
                let conns = &mut self.nodes[neighbor_id as usize].connections[layer];
                if conns.len() < self.m {
                    let _ = conns.push(node_id);
                }
            }
        }
    }

    /// Calculate similarity between two vectors (dot product / max possible)
    fn similarity(&self, a: &PatternVector, b: &PatternVector) -> f32 {
        let mut dot: i32 = 0;
        for (x, y) in a.iter().zip(b.iter()) {
            dot += (*x as i32) * (*y as i32);
        }
        // Normalize to [0, 1] range
        let max_dot = (127i32 * 127 * FEATURE_DIMS as i32) as f32;
        (dot as f32 + max_dot) / (2.0 * max_dot)
    }

    /// Generate random layer for new node
    fn random_layer(&mut self) -> u8 {
        // Simple LCG random
        self.random_state = self.random_state.wrapping_mul(1103515245).wrapping_add(12345);
        let r = (self.random_state >> 16) as f32 / 65536.0;

        // Exponential distribution
        let ml = 1.0 / libm::logf(self.m as f32);
        let layer = (-libm::logf(r) * ml) as u8;
        layer.min(MAX_LAYERS as u8 - 1)
    }

    /// Estimate memory usage in bytes
    fn estimate_memory(&self) -> usize {
        let node_size = core::mem::size_of::<HnswNode>();
        node_size * self.nodes.len()
    }

    /// Get a vector by ID
    pub fn get(&self, id: u32) -> Option<&PatternVector> {
        self.nodes.get(id as usize).filter(|n| n.active).map(|n| &n.vector)
    }

    /// Mark a vector as deleted (soft delete)
    pub fn delete(&mut self, id: u32) -> bool {
        if let Some(node) = self.nodes.get_mut(id as usize) {
            node.active = false;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vector(base: i8) -> PatternVector {
        let mut v = [0i8; FEATURE_DIMS];
        for i in 0..FEATURE_DIMS {
            v[i] = base.saturating_add((i as i8) * 5);
        }
        v
    }

    #[test]
    fn test_hnsw_insert() {
        let mut hnsw = MiniHnsw::new(8, 50);

        let id = hnsw.insert(make_vector(10)).unwrap();
        assert_eq!(id, 0);
        assert_eq!(hnsw.len(), 1);

        let id2 = hnsw.insert(make_vector(20)).unwrap();
        assert_eq!(id2, 1);
        assert_eq!(hnsw.len(), 2);
    }

    #[test]
    fn test_hnsw_search() {
        let mut hnsw = MiniHnsw::new(8, 50);

        // Insert several vectors
        for i in 0..10 {
            hnsw.insert(make_vector(i * 10)).unwrap();
        }

        // Search for vector similar to make_vector(50)
        let query = make_vector(50);
        let results = hnsw.search(&query, 3, 20);

        // Should find some results
        assert!(!results.is_empty());

        // First result should have high similarity
        assert!(results[0].1 > 0.5);
    }

    #[test]
    fn test_hnsw_similarity() {
        let hnsw = MiniHnsw::new(8, 50);

        let a = make_vector(50);
        let b = make_vector(50);
        let c = make_vector(-50);

        // Same vectors should have identical similarity (> 0.5 in normalized space)
        let sim_same = hnsw.similarity(&a, &b);
        assert!(sim_same > 0.5, "Same vector similarity: {}", sim_same);

        // Opposite vectors should have lower similarity than same vectors
        let sim_opp = hnsw.similarity(&a, &c);
        assert!(sim_opp < sim_same, "Opposite similarity {} should be < same {}", sim_opp, sim_same);
    }

    #[test]
    fn test_hnsw_stats() {
        let mut hnsw = MiniHnsw::new(8, 50);

        for i in 0..5 {
            hnsw.insert(make_vector(i * 10)).unwrap();
        }

        let stats = hnsw.stats();
        assert_eq!(stats.num_vectors, 5);
        assert!(stats.num_layers >= 1);
    }

    #[test]
    fn test_hnsw_delete() {
        let mut hnsw = MiniHnsw::new(8, 50);

        hnsw.insert(make_vector(10)).unwrap();
        hnsw.insert(make_vector(20)).unwrap();
        assert_eq!(hnsw.len(), 2);

        hnsw.delete(0);
        assert_eq!(hnsw.len(), 1);
    }
}
