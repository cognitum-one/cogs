//! HNSW vector index wrapper

use crate::ruvector::types::*;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(test)]
use mockall::automock;

/// Trait for vector indexing operations
#[cfg_attr(test, automock)]
pub trait VectorIndex: Send + Sync {
    /// Insert embedding with metadata
    fn insert(&mut self, id: EmbeddingId, embedding: &Embedding, metadata: &Metadata)
        -> Result<(), IndexError>;

    /// Search for k nearest neighbors
    fn search(&self, query: &Embedding, k: usize) -> Result<Vec<SearchResult>, IndexError>;

    /// Delete embedding by ID
    fn delete(&mut self, id: EmbeddingId) -> Result<(), IndexError>;

    /// Get index statistics
    fn stats(&self) -> IndexStats;

    /// Rebuild index for optimal performance
    fn optimize(&mut self) -> Result<(), IndexError>;
}

/// HNSW-based vector index implementation
pub struct HnswVectorIndex {
    dimension: usize,
    vectors: Arc<RwLock<HashMap<EmbeddingId, (Embedding, Metadata)>>>,
    #[allow(dead_code)] // Stored for future HNSW optimization
    config: HnswConfig,
}

#[derive(Debug, Clone)]
pub struct HnswConfig {
    pub m: usize,
    pub ef_construction: usize,
    pub ef_search: usize,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            m: 16,
            ef_construction: 200,
            ef_search: 50,
        }
    }
}

impl HnswVectorIndex {
    pub fn new(dimension: usize) -> Self {
        Self::with_config(dimension, HnswConfig::default())
    }

    pub fn with_config(dimension: usize, config: HnswConfig) -> Self {
        Self {
            dimension,
            vectors: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }
}

impl VectorIndex for HnswVectorIndex {
    fn insert(&mut self, id: EmbeddingId, embedding: &Embedding, metadata: &Metadata)
        -> Result<(), IndexError> {
        if embedding.dimension() != self.dimension {
            return Err(IndexError::InvalidDimension {
                expected: self.dimension,
                actual: embedding.dimension(),
            });
        }

        let mut vectors = self.vectors.write();
        vectors.insert(id, (embedding.clone(), metadata.clone()));
        Ok(())
    }

    fn search(&self, query: &Embedding, k: usize) -> Result<Vec<SearchResult>, IndexError> {
        if query.dimension() != self.dimension {
            return Err(IndexError::InvalidDimension {
                expected: self.dimension,
                actual: query.dimension(),
            });
        }

        let vectors = self.vectors.read();

        // Compute similarities for all vectors
        let mut results: Vec<(EmbeddingId, f32, Metadata)> = vectors
            .iter()
            .map(|(id, (emb, meta))| {
                let similarity = cosine_similarity(query, emb);
                (*id, similarity, meta.clone())
            })
            .collect();

        // Sort by similarity (descending)
        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top k
        Ok(results
            .into_iter()
            .take(k)
            .map(|(id, similarity, metadata)| SearchResult {
                id,
                similarity,
                metadata,
            })
            .collect())
    }

    fn delete(&mut self, id: EmbeddingId) -> Result<(), IndexError> {
        let mut vectors = self.vectors.write();
        vectors.remove(&id)
            .ok_or_else(|| IndexError::NotFound(format!("Embedding {:?} not found", id)))?;
        Ok(())
    }

    fn stats(&self) -> IndexStats {
        let vectors = self.vectors.read();
        IndexStats {
            num_vectors: vectors.len(),
            dimension: self.dimension,
            memory_bytes: vectors.len() * self.dimension * std::mem::size_of::<f32>(),
        }
    }

    fn optimize(&mut self) -> Result<(), IndexError> {
        // For simple implementation, optimization is a no-op
        // In production, this would rebuild HNSW graph
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_search() {
        let mut index = HnswVectorIndex::new(256);

        // Insert some embeddings
        for i in 0..10 {
            let embedding = Embedding::random(256);
            let metadata = Metadata::default();
            index.insert(EmbeddingId(i), &embedding, &metadata).unwrap();
        }

        // Search
        let query = Embedding::random(256);
        let results = index.search(&query, 5).unwrap();

        assert_eq!(results.len(), 5);
        // Results should be sorted by similarity
        for i in 1..results.len() {
            assert!(results[i - 1].similarity >= results[i].similarity);
        }
    }

    #[test]
    fn test_dimension_validation() {
        let mut index = HnswVectorIndex::new(256);

        let embedding = Embedding::random(128);
        let metadata = Metadata::default();

        let result = index.insert(EmbeddingId(1), &embedding, &metadata);
        assert!(matches!(result, Err(IndexError::InvalidDimension { .. })));
    }

    #[test]
    fn test_delete() {
        let mut index = HnswVectorIndex::new(256);

        let embedding = Embedding::random(256);
        let metadata = Metadata::default();
        index.insert(EmbeddingId(1), &embedding, &metadata).unwrap();

        assert_eq!(index.stats().num_vectors, 1);

        index.delete(EmbeddingId(1)).unwrap();
        assert_eq!(index.stats().num_vectors, 0);
    }
}
