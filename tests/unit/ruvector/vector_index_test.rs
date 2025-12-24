//! Unit tests for VectorIndex

#[cfg(test)]
mod vector_index {
    use cognitum::ruvector::{VectorIndex, HnswVectorIndex};
    use cognitum::ruvector::types::{Embedding, EmbeddingId, Metadata, IndexError};

    #[test]
    fn should_insert_and_retrieve() {
        let mut index = HnswVectorIndex::new(256);

        let embedding = Embedding::random(256);
        let metadata = Metadata::default();

        let result = index.insert(EmbeddingId(1), &embedding, &metadata);
        assert!(result.is_ok());

        let stats = index.stats();
        assert_eq!(stats.num_vectors, 1);
    }

    #[test]
    fn should_return_sorted_search_results() {
        let mut index = HnswVectorIndex::new(256);

        // Insert multiple embeddings
        for i in 0..20 {
            let embedding = Embedding::random(256);
            let metadata = Metadata::default();
            index.insert(EmbeddingId(i), &embedding, &metadata).unwrap();
        }

        // Search
        let query = Embedding::random(256);
        let results = index.search(&query, 10).unwrap();

        assert_eq!(results.len(), 10);

        // Results should be sorted by similarity (descending)
        for i in 1..results.len() {
            assert!(results[i - 1].similarity >= results[i].similarity);
        }
    }

    #[test]
    fn should_validate_dimension() {
        let mut index = HnswVectorIndex::new(256);

        let embedding = Embedding::random(128);
        let metadata = Metadata::default();

        let result = index.insert(EmbeddingId(1), &embedding, &metadata);

        assert!(matches!(result, Err(IndexError::InvalidDimension { .. })));
    }

    #[test]
    fn should_delete_embedding() {
        let mut index = HnswVectorIndex::new(256);

        let embedding = Embedding::random(256);
        let metadata = Metadata::default();
        index.insert(EmbeddingId(1), &embedding, &metadata).unwrap();

        assert_eq!(index.stats().num_vectors, 1);

        index.delete(EmbeddingId(1)).unwrap();
        assert_eq!(index.stats().num_vectors, 0);
    }

    #[test]
    fn should_handle_empty_search() {
        let index = HnswVectorIndex::new(256);

        let query = Embedding::random(256);
        let results = index.search(&query, 10).unwrap();

        assert_eq!(results.len(), 0);
    }

    #[test]
    fn should_optimize_without_error() {
        let mut index = HnswVectorIndex::new(256);

        for i in 0..100 {
            let embedding = Embedding::random(256);
            let metadata = Metadata::default();
            index.insert(EmbeddingId(i), &embedding, &metadata).unwrap();
        }

        let result = index.optimize();
        assert!(result.is_ok());
    }
}
