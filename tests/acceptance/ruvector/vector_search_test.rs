//! Acceptance tests for vector search

#[cfg(test)]
mod vector_search {
    use cognitum::ruvector::{CognitumRuvector, RuvectorConfig};
    use cognitum::ruvector::types::{TileState, Embedding};

    #[test]
    fn should_store_and_retrieve_embeddings() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        // Generate embeddings from multiple runs
        for _ in 0..50 {
            let states: Vec<TileState> = (0..4).map(|_| TileState::random()).collect();
            let embeddings = ruvector.capture_state(&states);
            ruvector.store_embeddings(&embeddings).unwrap();
        }

        // Search for similar states
        let query_state = TileState::random();
        let query_embeddings = ruvector.capture_state(&[query_state]);
        let results = ruvector.search_similar(&query_embeddings[0], 10).unwrap();

        assert_eq!(results.len(), 10);
        // Results should be sorted by similarity
        for i in 1..results.len() {
            assert!(
                results[i - 1].similarity >= results[i].similarity,
                "Results not sorted: {} < {}",
                results[i - 1].similarity,
                results[i].similarity
            );
        }
    }

    #[test]
    fn should_find_similar_states() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        // Store a specific state
        let target_state = TileState {
            program_counter: 5000,
            stack_pointer: 200,
            registers: [100; 32],
            cycle_count: 10000,
            message_count: 50,
        };

        let target_emb = ruvector.capture_state(&[target_state]);
        ruvector.store_embeddings(&target_emb).unwrap();

        // Store many random states
        for _ in 0..100 {
            let states: Vec<TileState> = (0..4).map(|_| TileState::random()).collect();
            let embeddings = ruvector.capture_state(&states);
            ruvector.store_embeddings(&embeddings).unwrap();
        }

        // Search for similar state
        let query_state = TileState {
            program_counter: 5001, // Very close to target
            stack_pointer: 200,
            registers: [100; 32],
            cycle_count: 10001,
            message_count: 50,
        };

        let query_emb = ruvector.capture_state(&[query_state]);
        let results = ruvector.search_similar(&query_emb[0], 1).unwrap();

        // The most similar should be very close to our target
        assert!(results[0].similarity > 0.9, "Similarity: {}", results[0].similarity);
    }

    #[test]
    fn should_scale_to_many_vectors() {
        let config = RuvectorConfig {
            index_capacity: 10_000,
            ..Default::default()
        };
        let ruvector = CognitumRuvector::new(config);

        // Insert 1000 random vectors
        let num_vectors = 1000;
        for _ in 0..num_vectors / 10 {
            let embeddings: Vec<Embedding> = (0..10).map(|_| Embedding::random(256)).collect();
            ruvector.store_embeddings(&embeddings).unwrap();
        }

        // Verify all stored
        let stats = ruvector.index_stats();
        assert_eq!(stats.num_vectors, num_vectors);

        // Search should be fast
        let query = Embedding::random(256);
        let start = std::time::Instant::now();
        let results = ruvector.search_similar(&query, 10).unwrap();
        let duration = start.elapsed();

        assert_eq!(results.len(), 10);
        println!("Search on {} vectors took: {:?}", num_vectors, duration);

        // Should be reasonably fast even with simple implementation
        assert!(duration.as_millis() < 100, "Search too slow: {:?}", duration);
    }

    #[test]
    fn should_handle_empty_index_search() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        let query = Embedding::random(256);
        let results = ruvector.search_similar(&query, 10).unwrap();

        assert_eq!(results.len(), 0);
    }

    #[test]
    fn should_return_fewer_results_when_index_small() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        // Store only 3 embeddings
        let embeddings: Vec<Embedding> = (0..3).map(|_| Embedding::random(256)).collect();
        ruvector.store_embeddings(&embeddings).unwrap();

        // Request 10 but should get 3
        let query = Embedding::random(256);
        let results = ruvector.search_similar(&query, 10).unwrap();

        assert_eq!(results.len(), 3);
    }
}
