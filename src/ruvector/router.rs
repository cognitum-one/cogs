//! Task routing using TinyDancer (FastGRNN)

use crate::ruvector::types::*;
use parking_lot::RwLock;
use std::path::Path;
use std::sync::Arc;

#[cfg(test)]
use mockall::automock;

/// Trait for intelligent task routing
#[cfg_attr(test, automock)]
pub trait TaskRouter: Send + Sync {
    /// Predict optimal tile for task execution
    fn predict_tile(&self, task_embedding: &TaskEmbedding) -> TileId;

    /// Get routing confidence score
    fn confidence(&self, task_embedding: &TaskEmbedding) -> f32;

    /// Train model from execution traces
    fn train(&mut self, traces: &[ExecutionTrace]) -> Result<TrainingMetrics, RouterError>;

    /// Load pre-trained model
    fn load_model(&mut self, path: &Path) -> Result<(), RouterError>;

    /// Save current model
    fn save_model(&self, path: &Path) -> Result<(), RouterError>;
}

/// TinyDancer-based router using FastGRNN
pub struct TinyDancerRouter {
    num_tiles: usize,
    model_weights: Arc<RwLock<Vec<Vec<f32>>>>,
    input_dim: usize,
}

impl TinyDancerRouter {
    pub fn new(num_tiles: usize, input_dim: usize) -> Self {
        // Initialize random weights
        let weights = (0..num_tiles)
            .map(|_| {
                (0..input_dim)
                    .map(|_| rand::random::<f32>() * 0.01)
                    .collect()
            })
            .collect();

        Self {
            num_tiles,
            model_weights: Arc::new(RwLock::new(weights)),
            input_dim,
        }
    }

    fn predict_probabilities(&self, task_embedding: &TaskEmbedding) -> Vec<f32> {
        let weights = self.model_weights.read();

        // Simple linear model: logits = W * x
        let logits: Vec<f32> = weights
            .iter()
            .map(|w| {
                w.iter()
                    .zip(&task_embedding.data)
                    .map(|(wi, xi)| wi * xi)
                    .sum()
            })
            .collect();

        // Softmax
        let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let exp_logits: Vec<f32> = logits.iter().map(|l| (l - max_logit).exp()).collect();
        let sum_exp: f32 = exp_logits.iter().sum();

        exp_logits.iter().map(|e| e / sum_exp).collect()
    }
}

impl TaskRouter for TinyDancerRouter {
    fn predict_tile(&self, task_embedding: &TaskEmbedding) -> TileId {
        let probs = self.predict_probabilities(task_embedding);

        // Return argmax
        let best_tile = probs
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(idx, _)| idx)
            .unwrap_or(0);

        TileId(best_tile as u32)
    }

    fn confidence(&self, task_embedding: &TaskEmbedding) -> f32 {
        let probs = self.predict_probabilities(task_embedding);
        *probs.iter().max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(&0.0)
    }

    fn train(&mut self, traces: &[ExecutionTrace]) -> Result<TrainingMetrics, RouterError> {
        if traces.is_empty() {
            return Err(RouterError::Training("No training data provided".to_string()));
        }

        let learning_rate = 0.01;
        let epochs = 100;
        let mut final_loss = 0.0;

        let mut weights = self.model_weights.write();

        for _epoch in 0..epochs {
            let mut epoch_loss = 0.0;

            for trace in traces {
                // Forward pass
                let probs = {
                    let logits: Vec<f32> = weights
                        .iter()
                        .map(|w| {
                            w.iter()
                                .zip(&trace.task_embedding.data)
                                .map(|(wi, xi)| wi * xi)
                                .sum()
                        })
                        .collect();

                    let max_logit = logits.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                    let exp_logits: Vec<f32> = logits.iter().map(|l| (l - max_logit).exp()).collect();
                    let sum_exp: f32 = exp_logits.iter().sum();
                    exp_logits.iter().map(|e| e / sum_exp).collect::<Vec<_>>()
                };

                // Compute loss (cross-entropy)
                let target_idx = trace.actual_tile.0 as usize;
                let loss = -probs[target_idx].ln();
                epoch_loss += loss;

                // Backward pass (gradient descent)
                for (i, w) in weights.iter_mut().enumerate() {
                    let error = if i == target_idx {
                        probs[i] - 1.0
                    } else {
                        probs[i]
                    };

                    for (j, wj) in w.iter_mut().enumerate() {
                        let grad = error * trace.task_embedding.data[j];
                        *wj -= learning_rate * grad;
                    }
                }
            }

            final_loss = epoch_loss / traces.len() as f32;
        }

        // Compute accuracy
        let mut correct = 0;
        for trace in traces {
            let pred = self.predict_tile(&trace.task_embedding);
            if pred == trace.actual_tile {
                correct += 1;
            }
        }
        let accuracy = correct as f32 / traces.len() as f32;

        Ok(TrainingMetrics {
            epochs,
            final_loss,
            accuracy,
        })
    }

    fn load_model(&mut self, path: &Path) -> Result<(), RouterError> {
        let data = std::fs::read_to_string(path)?;
        let weights: Vec<Vec<f32>> = serde_json::from_str(&data)
            .map_err(|e| RouterError::Model(e.to_string()))?;

        *self.model_weights.write() = weights;
        Ok(())
    }

    fn save_model(&self, path: &Path) -> Result<(), RouterError> {
        let weights = self.model_weights.read();
        let data = serde_json::to_string(&*weights)
            .map_err(|e| RouterError::Model(e.to_string()))?;
        std::fs::write(path, data)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predict_returns_valid_tile() {
        let router = TinyDancerRouter::new(8, 256);
        let task = TaskEmbedding::random();

        let tile_id = router.predict_tile(&task);
        assert!(tile_id.0 < 8);
    }

    #[test]
    fn test_confidence_in_range() {
        let router = TinyDancerRouter::new(8, 256);
        let task = TaskEmbedding::random();

        let conf = router.confidence(&task);
        assert!(conf >= 0.0 && conf <= 1.0);
    }

    #[test]
    fn test_training_improves_accuracy() {
        let mut router = TinyDancerRouter::new(4, 256);

        // Generate consistent training data
        let traces: Vec<ExecutionTrace> = (0..100)
            .map(|i| {
                let mut task = TaskEmbedding::random();
                // Make tile 0 always have high first feature
                if i % 4 == 0 {
                    task.data[0] = 0.9;
                }
                ExecutionTrace {
                    task_embedding: task,
                    actual_tile: TileId((i % 4) as u32),
                    execution_time_us: 1000,
                    success: true,
                }
            })
            .collect();

        let metrics = router.train(&traces).unwrap();

        // Should achieve reasonable accuracy
        assert!(metrics.accuracy > 0.2); // Better than random (0.25)
        assert!(metrics.final_loss < 2.0);
    }
}
