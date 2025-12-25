//! Acceptance tests for embedding generation

#[cfg(test)]
mod embedding_generation {
    use cognitum::ruvector::{CognitumRuvector, RuvectorConfig};
    use cognitum::ruvector::types::{TileState, cosine_similarity};

    #[test]
    fn should_generate_embedding_from_tile_state() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        // Create some tile states
        let states: Vec<TileState> = (0..16).map(|_| TileState::random()).collect();

        // Capture embeddings
        let embeddings = ruvector.capture_state(&states);

        // Should have one embedding per tile
        assert_eq!(embeddings.len(), 16);

        // Each embedding should have correct dimension
        for emb in &embeddings {
            assert_eq!(emb.dimension(), 256);
        }
    }

    #[test]
    fn should_produce_similar_embeddings_for_similar_states() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        // Create deterministic state
        let state1 = TileState {
            program_counter: 1000,
            stack_pointer: 100,
            registers: [42; 32],
            cycle_count: 5000,
            message_count: 10,
        };

        let state2 = TileState {
            program_counter: 1001, // Slightly different
            stack_pointer: 100,
            registers: [42; 32],
            cycle_count: 5001,
            message_count: 10,
        };

        let emb1 = ruvector.capture_state(&[state1]);
        let emb2 = ruvector.capture_state(&[state2]);

        // Embeddings should be very similar
        let similarity = cosine_similarity(&emb1[0], &emb2[0]);
        assert!(similarity > 0.95, "Similarity: {}", similarity);
    }

    #[test]
    fn should_batch_generate_efficiently() {
        let config = RuvectorConfig {
            num_tiles: 64,
            ..Default::default()
        };
        let ruvector = CognitumRuvector::new(config);

        let states: Vec<TileState> = (0..64).map(|_| TileState::random()).collect();

        let start = std::time::Instant::now();
        let embeddings = ruvector.capture_state(&states);
        let duration = start.elapsed();

        assert_eq!(embeddings.len(), 64);

        // Should be fast (< 1ms for 64 tiles = < 15.6μs per tile)
        println!("Batch generation took: {:?}", duration);
        println!("Per tile: {:?}", duration / 64);
        assert!(duration.as_millis() < 10, "Too slow: {:?}", duration);
    }

    #[test]
    fn should_encode_different_states_differently() {
        let config = RuvectorConfig::default();
        let ruvector = CognitumRuvector::new(config);

        let state1 = TileState {
            program_counter: 0,
            stack_pointer: 0,
            registers: [0; 32],
            cycle_count: 0,
            message_count: 0,
        };

        let state2 = TileState {
            program_counter: u32::MAX,
            stack_pointer: 4095,
            registers: [255; 32],
            cycle_count: u64::MAX,
            message_count: u32::MAX,
        };

        let emb1 = ruvector.capture_state(&[state1]);
        let emb2 = ruvector.capture_state(&[state2]);

        // Embeddings should be very different
        let similarity = cosine_similarity(&emb1[0], &emb2[0]);
        assert!(similarity < 0.9, "Too similar: {}", similarity);
    }
}
