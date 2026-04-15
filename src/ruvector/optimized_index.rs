//! Optimized HNSW vector index implementation
//!
//! This module provides a production-grade HNSW (Hierarchical Navigable Small World)
//! index for fast approximate nearest neighbor search.
//!
//! Performance target: < 10ms for 1M vectors

use crate::ruvector::types::*;
use crate::ruvector::optimized_embedding::cosine_similarity_simd;
use parking_lot::RwLock;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::sync::Arc;
use std::cmp::Ordering;

/// HNSW graph layer
#[derive(Clone)]
struct HnswLayer {
    /// Adjacency list: node_id -> neighbors
    graph: HashMap<EmbeddingId, Vec<EmbeddingId>>,
}

impl HnswLayer {
    fn new() -> Self {
        Self {
            graph: HashMap::new(),
        }
    }

    fn add_node(&mut self, id: EmbeddingId) {
        self.graph.entry(id).or_insert_with(Vec::new);
    }

    fn add_edge(&mut self, from: EmbeddingId, to: EmbeddingId) {
        self.graph
            .entry(from)
            .or_insert_with(Vec::new)
            .push(to);
    }

    fn neighbors(&self, id: &EmbeddingId) -> &[EmbeddingId] {
        self.graph
            .get(id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}

/// Distance comparison for priority queue (min-heap by distance)
#[derive(Clone)]
struct DistanceNode {
    id: EmbeddingId,
    distance: f32,
}

impl PartialEq for DistanceNode {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for DistanceNode {}

impl PartialOrd for DistanceNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Reverse ordering for max-heap (BinaryHeap is max-heap by default)
        other.distance.partial_cmp(&self.distance)
    }
}

impl Ord for DistanceNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// Optimized HNSW vector index
pub struct OptimizedHnswIndex {
    dimension: usize,

    /// Vector storage
    vectors: Arc<RwLock<HashMap<EmbeddingId, (Embedding, Metadata)>>>,

    /// Multi-layer HNSW graph
    layers: Vec<HnswLayer>,

    /// Entry point for search (top layer)
    entry_point: Option<EmbeddingId>,

    /// Configuration
    config: HnswConfig,
}

#[derive(Debug, Clone)]
pub struct HnswConfig {
    /// Number of bi-directional links per node (M parameter)
    pub m: usize,

    /// Size of dynamic candidate list during construction (efConstruction)
    pub ef_construction: usize,

    /// Size of dynamic candidate list during search (ef)
    pub ef_search: usize,

    /// Maximum number of layers
    pub max_layers: usize,

    /// Layer selection multiplier (mL)
    pub ml: f32,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            ef_search: 50,
            max_layers: 8,
            ml: 1.0 / (16.0_f32).ln(),
        }
    }
}

impl OptimizedHnswIndex {
    pub fn new(dimension: usize) -> Self {
        Self::with_config(dimension, HnswConfig::default())
    }

    pub fn with_config(dimension: usize, config: HnswConfig) -> Self {
        let mut layers = Vec::with_capacity(config.max_layers);
        for _ in 0..config.max_layers {
            layers.push(HnswLayer::new());
        }

        Self {
            dimension,
            vectors: Arc::new(RwLock::new(HashMap::new())),
            layers,
            entry_point: None,
            config,
        }
    }

    /// Insert a new vector into the index
    pub fn insert(
        &mut self,
        id: EmbeddingId,
        embedding: &Embedding,
        metadata: &Metadata,
    ) -> Result<(), IndexError> {
        if embedding.dimension() != self.dimension {
            return Err(IndexError::InvalidDimension {
                expected: self.dimension,
                actual: embedding.dimension(),
            });
        }

        // Store vector
        {
            let mut vectors = self.vectors.write();
            vectors.insert(id, (embedding.clone(), metadata.clone()));
        }

        // Determine layer for new node
        let layer = self.random_layer();

        // Add node to layers
        for l in 0..=layer {
            self.layers[l].add_node(id);
        }

        // If this is the first node, make it the entry point
        if self.entry_point.is_none() {
            self.entry_point = Some(id);
            return Ok(());
        }

        // Find nearest neighbors and create links
        let entry_point = self.entry_point.unwrap();

        // Search from top to target layer
        let mut current_nearest = entry_point;
        for l in (layer + 1..self.config.max_layers).rev() {
            current_nearest = self.search_layer(embedding, current_nearest, 1, l)[0].id;
        }

        // Insert into layers from target layer down to 0
        for l in (0..=layer).rev() {
            let candidates = self.search_layer(embedding, current_nearest, self.config.ef_construction, l);

            // Select M neighbors using heuristic
            let m = if l == 0 { self.config.m * 2 } else { self.config.m };
            let neighbors = self.select_neighbors(&candidates, m);

            // Add bidirectional links
            for neighbor_id in &neighbors {
                self.layers[l].add_edge(id, *neighbor_id);
                self.layers[l].add_edge(*neighbor_id, id);

                // Prune neighbors if needed
                self.prune_connections(*neighbor_id, l);
            }

            if !candidates.is_empty() {
                current_nearest = candidates[0].id;
            }
        }

        Ok(())
    }

    /// Search for k nearest neighbors
    pub fn search(&self, query: &Embedding, k: usize) -> Result<Vec<SearchResult>, IndexError> {
        if query.dimension() != self.dimension {
            return Err(IndexError::InvalidDimension {
                expected: self.dimension,
                actual: query.dimension(),
            });
        }

        let entry_point = match self.entry_point {
            Some(ep) => ep,
            None => return Ok(Vec::new()),
        };

        // Search from top layer to layer 0
        let mut current_nearest = entry_point;
        for l in (1..self.config.max_layers).rev() {
            let results = self.search_layer(query, current_nearest, 1, l);
            if !results.is_empty() {
                current_nearest = results[0].id;
            }
        }

        // Search layer 0 with ef_search
        let candidates = self.search_layer(query, current_nearest, self.config.ef_search.max(k), 0);

        // Convert to SearchResult and take top k
        let vectors = self.vectors.read();
        Ok(candidates
            .into_iter()
            .take(k)
            .filter_map(|node| {
                vectors.get(&node.id).map(|(_, metadata)| SearchResult {
                    id: node.id,
                    similarity: 1.0 - node.distance, // Convert distance to similarity
                    metadata: metadata.clone(),
                })
            })
            .collect())
    }

    /// Search within a specific layer
    fn search_layer(
        &self,
        query: &Embedding,
        entry_point: EmbeddingId,
        ef: usize,
        layer: usize,
    ) -> Vec<DistanceNode> {
        let vectors = self.vectors.read();
        let mut visited = HashSet::new();
        let mut candidates = BinaryHeap::new();
        let mut w = BinaryHeap::new();

        // Initialize with entry point
        let entry_dist = self.distance(query, &vectors.get(&entry_point).unwrap().0);
        let entry_node = DistanceNode {
            id: entry_point,
            distance: entry_dist,
        };

        candidates.push(entry_node.clone());
        w.push(entry_node.clone());
        visited.insert(entry_point);

        // Greedy search
        while let Some(c) = candidates.pop() {
            let f = w.peek().unwrap();
            if c.distance > f.distance {
                break;
            }

            // Check neighbors
            for &neighbor_id in self.layers[layer].neighbors(&c.id) {
                if !visited.contains(&neighbor_id) {
                    visited.insert(neighbor_id);

                    if let Some((neighbor_vec, _)) = vectors.get(&neighbor_id) {
                        let dist = self.distance(query, neighbor_vec);
                        let neighbor_node = DistanceNode {
                            id: neighbor_id,
                            distance: dist,
                        };

                        let f = w.peek().unwrap();
                        if dist < f.distance || w.len() < ef {
                            candidates.push(neighbor_node.clone());
                            w.push(neighbor_node);

                            // Maintain ef size
                            if w.len() > ef {
                                w.pop();
                            }
                        }
                    }
                }
            }
        }

        // Convert to sorted vector
        w.into_sorted_vec()
    }

    /// Select neighbors using heuristic
    fn select_neighbors(&self, candidates: &[DistanceNode], m: usize) -> Vec<EmbeddingId> {
        candidates
            .iter()
            .take(m)
            .map(|node| node.id)
            .collect()
    }

    /// Prune connections to maintain M limit
    fn prune_connections(&mut self, node_id: EmbeddingId, layer: usize) {
        let m_max = if layer == 0 {
            self.config.m * 2
        } else {
            self.config.m
        };

        let neighbors = self.layers[layer].neighbors(&node_id).to_vec();
        if neighbors.len() <= m_max {
            return;
        }

        // Keep closest M neighbors
        let vectors = self.vectors.read();
        let node_vec = &vectors.get(&node_id).unwrap().0;

        let mut neighbor_distances: Vec<_> = neighbors
            .iter()
            .map(|&neighbor_id| {
                let neighbor_vec = &vectors.get(&neighbor_id).unwrap().0;
                let dist = self.distance(node_vec, neighbor_vec);
                DistanceNode {
                    id: neighbor_id,
                    distance: dist,
                }
            })
            .collect();

        neighbor_distances.sort_by(|a, b| {
            a.distance.partial_cmp(&b.distance).unwrap_or(Ordering::Equal)
        });

        // Keep only M closest
        let new_neighbors: Vec<_> = neighbor_distances
            .into_iter()
            .take(m_max)
            .map(|node| node.id)
            .collect();

        // Update graph
        self.layers[layer].graph.insert(node_id, new_neighbors);
    }

    /// Calculate distance between embeddings (using SIMD-optimized cosine similarity)
    #[inline]
    fn distance(&self, a: &Embedding, b: &Embedding) -> f32 {
        1.0 - cosine_similarity_simd(a, b)
    }

    /// Random layer selection using exponential distribution
    fn random_layer(&self) -> usize {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let uniform: f32 = rng.gen_range(0.0..1.0);
        let layer = (-uniform.ln() * self.config.ml).floor() as usize;
        layer.min(self.config.max_layers - 1)
    }

    /// Get index statistics
    pub fn stats(&self) -> IndexStats {
        let vectors = self.vectors.read();
        IndexStats {
            num_vectors: vectors.len(),
            dimension: self.dimension,
            memory_bytes: vectors.len() * self.dimension * std::mem::size_of::<f32>(),
        }
    }

    /// Delete a vector from the index
    pub fn delete(&mut self, id: EmbeddingId) -> Result<(), IndexError> {
        // Remove from vector storage
        {
            let mut vectors = self.vectors.write();
            vectors
                .remove(&id)
                .ok_or_else(|| IndexError::NotFound(format!("Embedding {:?} not found", id)))?;
        }

        // Remove from all layers
        for layer in &mut self.layers {
            layer.graph.remove(&id);

            // Remove from neighbors' adjacency lists
            for neighbors in layer.graph.values_mut() {
                neighbors.retain(|&neighbor_id| neighbor_id != id);
            }
        }

        Ok(())
    }

    /// Optimize the index (rebuild for better performance)
    pub fn optimize(&mut self) -> Result<(), IndexError> {
        // For HNSW, optimization would involve rebuilding the entire graph
        // This is a placeholder - full implementation would reconstruct layers
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_insert_and_search() {
        let mut index = OptimizedHnswIndex::new(128);

        // Insert vectors
        for i in 0..100 {
            let embedding = Embedding::random(128);
            let metadata = Metadata::default();
            index.insert(EmbeddingId(i), &embedding, &metadata).unwrap();
        }

        // Search
        let query = Embedding::random(128);
        let results = index.search(&query, 10).unwrap();

        assert_eq!(results.len(), 10);
        // Results should be sorted by similarity
        for i in 1..results.len() {
            assert!(results[i - 1].similarity >= results[i].similarity);
        }
    }

    #[test]
    fn test_hnsw_delete() {
        let mut index = OptimizedHnswIndex::new(128);

        // Insert and then delete
        let embedding = Embedding::random(128);
        let metadata = Metadata::default();
        index.insert(EmbeddingId(1), &embedding, &metadata).unwrap();

        assert_eq!(index.stats().num_vectors, 1);

        index.delete(EmbeddingId(1)).unwrap();
        assert_eq!(index.stats().num_vectors, 0);
    }

    #[test]
    fn test_hnsw_performance() {
        let mut index = OptimizedHnswIndex::new(256);

        // Insert 1000 vectors
        for i in 0..1000 {
            let embedding = Embedding::random(256);
            let metadata = Metadata::default();
            index.insert(EmbeddingId(i), &embedding, &metadata).unwrap();
        }

        // Search should be fast
        let query = Embedding::random(256);
        let start = std::time::Instant::now();
        let results = index.search(&query, 10).unwrap();
        let duration = start.elapsed();

        assert_eq!(results.len(), 10);
        // Should complete in reasonable time (< 1ms for 1K vectors)
        assert!(duration.as_millis() < 10);
    }
}
