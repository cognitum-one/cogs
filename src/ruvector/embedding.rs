//! Embedding generation from tile and processor states

use crate::ruvector::types::*;

#[cfg(test)]
use mockall::automock;

/// Trait for generating embeddings from chip states
#[cfg_attr(test, automock)]
pub trait EmbeddingGenerator: Send + Sync {
    /// Generate embedding from single tile state
    fn from_tile_state(&self, state: &TileState) -> Embedding;

    /// Generate embedding from processor state (all registers)
    fn from_processor_state(&self, state: &ProcessorState) -> Embedding;

    /// Batch generate embeddings for all tiles
    fn batch_generate(&self, states: &[TileState]) -> Vec<Embedding>;

    /// Get embedding dimension
    fn dimension(&self) -> usize;
}

/// Default implementation of embedding generator
pub struct DefaultEmbeddingGenerator {
    dimension: usize,
}

impl DefaultEmbeddingGenerator {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

impl EmbeddingGenerator for DefaultEmbeddingGenerator {
    fn from_tile_state(&self, state: &TileState) -> Embedding {
        let mut data = vec![0.0; self.dimension];

        // Normalize program counter to [0, 1]
        data[0] = (state.program_counter as f64 / u32::MAX as f64) as f32;

        // Normalize stack pointer to [0, 1]
        data[1] = (state.stack_pointer as f32) / 4096.0;

        // Encode register values (normalized to [0, 1])
        let reg_start = 2;
        for (i, &reg_val) in state.registers.iter().enumerate().take(self.dimension - reg_start - 2) {
            data[reg_start + i] = (reg_val as f32) / 255.0;
        }

        // Temporal features (last two dimensions)
        if self.dimension >= 2 {
            let cycle_norm = (state.cycle_count as f64).ln() / 20.0; // log normalization
            data[self.dimension - 2] = cycle_norm.min(1.0).max(0.0) as f32;

            let msg_norm = (state.message_count as f32).ln() / 10.0;
            data[self.dimension - 1] = msg_norm.min(1.0).max(0.0);
        }

        Embedding::new(data)
    }

    fn from_processor_state(&self, state: &ProcessorState) -> Embedding {
        if state.tiles.is_empty() {
            return Embedding::zeros(self.dimension);
        }

        // Aggregate all tile states
        let embeddings: Vec<Embedding> = state.tiles
            .iter()
            .map(|tile| self.from_tile_state(tile))
            .collect();

        // Average embeddings
        let mut avg = vec![0.0; self.dimension];
        for emb in &embeddings {
            for (i, &val) in emb.data.iter().enumerate() {
                avg[i] += val;
            }
        }
        let count = embeddings.len() as f32;
        for val in &mut avg {
            *val /= count;
        }

        Embedding::new(avg)
    }

    fn batch_generate(&self, states: &[TileState]) -> Vec<Embedding> {
        states.iter().map(|state| self.from_tile_state(state)).collect()
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_program_counter() {
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
    fn test_encode_register_values() {
        let generator = DefaultEmbeddingGenerator::new(256);

        let mut state = TileState::default();
        state.registers[0] = 255;
        state.registers[1] = 0;

        let embedding = generator.from_tile_state(&state);

        // Register values should appear in embedding
        let reg_start = 2;
        assert!((embedding.data[reg_start] - 1.0).abs() < 0.01); // 255 -> 1.0
        assert!((embedding.data[reg_start + 1] - 0.0).abs() < 0.01); // 0 -> 0.0
    }

    #[test]
    fn test_consistent_dimension() {
        let generator = DefaultEmbeddingGenerator::new(128);

        for _ in 0..100 {
            let state = TileState::random();
            let embedding = generator.from_tile_state(&state);
            assert_eq!(embedding.dimension(), 128);
        }
    }
}
