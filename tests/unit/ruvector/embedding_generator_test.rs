//! Unit tests for EmbeddingGenerator

#[cfg(test)]
mod embedding_generator {
    use cognitum::ruvector::{EmbeddingGenerator, DefaultEmbeddingGenerator};
    use cognitum::ruvector::types::TileState;

    #[test]
    fn should_normalize_program_counter() {
        let generator = DefaultEmbeddingGenerator::new(256);

        let state = TileState {
            program_counter: 32768, // Mid-range PC
            stack_pointer: 128,
            ..Default::default()
        };

        let embedding = generator.from_tile_state(&state);

        // PC should be normalized to [0, 1]
        assert!(embedding.data[0] >= 0.0 && embedding.data[0] <= 1.0);
        assert!((embedding.data[0] - 0.5).abs() < 0.01); // Should be ~0.5
    }

    #[test]
    fn should_encode_register_values() {
        let generator = DefaultEmbeddingGenerator::new(256);

        let mut state = TileState::default();
        state.registers[0] = 255;
        state.registers[1] = 0;

        let embedding = generator.from_tile_state(&state);

        // Register values should appear in embedding
        let reg_start = 2; // After PC and SP
        assert!((embedding.data[reg_start] - 1.0).abs() < 0.01); // 255 -> 1.0
        assert!((embedding.data[reg_start + 1] - 0.0).abs() < 0.01); // 0 -> 0.0
    }

    #[test]
    fn should_include_temporal_features() {
        let generator = DefaultEmbeddingGenerator::new(256);

        let state = TileState {
            cycle_count: 1_000_000,
            message_count: 500,
            ..Default::default()
        };

        let embedding = generator.from_tile_state(&state);

        // Last features should be temporal
        let dim = embedding.dimension();
        assert!(embedding.data[dim - 2] > 0.0); // Cycle count encoded
        assert!(embedding.data[dim - 1] > 0.0); // Message count encoded
    }

    #[test]
    fn should_produce_consistent_dimension() {
        let generator = DefaultEmbeddingGenerator::new(128);

        for _ in 0..100 {
            let state = TileState::random();
            let embedding = generator.from_tile_state(&state);
            assert_eq!(embedding.dimension(), 128);
        }
    }

    #[test]
    fn should_batch_generate_match_individual() {
        let generator = DefaultEmbeddingGenerator::new(256);

        let states: Vec<TileState> = (0..5).map(|_| TileState::random()).collect();

        // Batch generate
        let batch_embeddings = generator.batch_generate(&states);

        // Individual generate
        let individual_embeddings: Vec<_> = states.iter()
            .map(|s| generator.from_tile_state(s))
            .collect();

        assert_eq!(batch_embeddings.len(), individual_embeddings.len());

        for (batch, individual) in batch_embeddings.iter().zip(individual_embeddings.iter()) {
            assert_eq!(batch.data, individual.data);
        }
    }
}
