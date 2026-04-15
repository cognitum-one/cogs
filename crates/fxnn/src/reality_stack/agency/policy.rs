//! Policy implementations for agent decision-making

use super::SensorReading;
use crate::error::Result;
use serde::{Deserialize, Serialize};

/// Output from a policy decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyOutput {
    /// Action intensities for each actuator [-1, 1]
    pub action_intensities: Vec<f32>,
    /// Confidence in the decision [0, 1]
    pub confidence: f32,
    /// Rationale for the decision
    pub rationale: String,
    /// Internal state update (for stateful policies)
    pub state_update: Option<Vec<f32>>,
}

/// Trait for policy implementations
pub trait Policy: Send + Sync {
    /// Get policy name
    fn name(&self) -> &str;

    /// Decide on actions given sensor readings and goal values
    fn decide(
        &self,
        sensor_readings: &[SensorReading],
        goal_values: &[f32],
    ) -> Result<PolicyOutput>;

    /// Update internal state (for learning policies)
    fn update(&mut self, _reward: f32) {
        // Default: no learning
    }

    /// Reset internal state
    fn reset(&mut self) {
        // Default: no state
    }

    /// Get number of actuator outputs
    fn n_outputs(&self) -> usize;

    /// Check if policy is deterministic
    fn is_deterministic(&self) -> bool {
        true
    }
}

/// Random policy for baseline/exploration
pub struct RandomPolicy {
    n_outputs: usize,
    action_scale: f32,
}

impl RandomPolicy {
    /// Create a new random policy
    pub fn new() -> Self {
        Self {
            n_outputs: 3,
            action_scale: 1.0,
        }
    }

    /// Set number of outputs
    pub fn with_n_outputs(mut self, n: usize) -> Self {
        self.n_outputs = n;
        self
    }

    /// Set action scale
    pub fn with_action_scale(mut self, scale: f32) -> Self {
        self.action_scale = scale;
        self
    }
}

impl Default for RandomPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl Policy for RandomPolicy {
    fn name(&self) -> &str {
        "RandomPolicy"
    }

    fn decide(
        &self,
        _sensor_readings: &[SensorReading],
        _goal_values: &[f32],
    ) -> Result<PolicyOutput> {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let intensities: Vec<f32> = (0..self.n_outputs)
            .map(|_| rng.gen_range(-self.action_scale..self.action_scale))
            .collect();

        Ok(PolicyOutput {
            action_intensities: intensities,
            confidence: 0.0, // Random has no confidence
            rationale: "Random action".to_string(),
            state_update: None,
        })
    }

    fn n_outputs(&self) -> usize {
        self.n_outputs
    }

    fn is_deterministic(&self) -> bool {
        false
    }
}

/// Rule-based policy using explicit rules
pub struct RuleBasedPolicy {
    rules: Vec<Rule>,
    default_output: Vec<f32>,
}

/// A rule for the rule-based policy
pub struct Rule {
    /// Condition function
    condition: Box<dyn Fn(&[SensorReading], &[f32]) -> bool + Send + Sync>,
    /// Action to take if condition is true
    action: Vec<f32>,
    /// Priority (higher = checked first)
    priority: i32,
    /// Rule name for debugging
    name: String,
}

impl Rule {
    /// Create a new rule
    pub fn new<F>(name: &str, condition: F, action: Vec<f32>) -> Self
    where
        F: Fn(&[SensorReading], &[f32]) -> bool + Send + Sync + 'static,
    {
        Self {
            condition: Box::new(condition),
            action,
            priority: 0,
            name: name.to_string(),
        }
    }

    /// Set priority
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Check if condition is met
    pub fn check(&self, readings: &[SensorReading], goals: &[f32]) -> bool {
        (self.condition)(readings, goals)
    }
}

impl RuleBasedPolicy {
    /// Create a new rule-based policy
    pub fn new(n_outputs: usize) -> Self {
        Self {
            rules: Vec::new(),
            default_output: vec![0.0; n_outputs],
        }
    }

    /// Add a rule
    pub fn with_rule(mut self, rule: Rule) -> Self {
        self.rules.push(rule);
        // Sort by priority (descending)
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
        self
    }

    /// Set default output
    pub fn with_default(mut self, output: Vec<f32>) -> Self {
        self.default_output = output;
        self
    }
}

impl Policy for RuleBasedPolicy {
    fn name(&self) -> &str {
        "RuleBasedPolicy"
    }

    fn decide(
        &self,
        sensor_readings: &[SensorReading],
        goal_values: &[f32],
    ) -> Result<PolicyOutput> {
        // Find first matching rule
        for rule in &self.rules {
            if rule.check(sensor_readings, goal_values) {
                return Ok(PolicyOutput {
                    action_intensities: rule.action.clone(),
                    confidence: 1.0,
                    rationale: format!("Rule matched: {}", rule.name),
                    state_update: None,
                });
            }
        }

        // No rule matched, use default
        Ok(PolicyOutput {
            action_intensities: self.default_output.clone(),
            confidence: 0.5,
            rationale: "Default action (no rule matched)".to_string(),
            state_update: None,
        })
    }

    fn n_outputs(&self) -> usize {
        self.default_output.len()
    }
}

/// Neural network policy (placeholder for integration with ML frameworks)
pub struct NeuralPolicy {
    name: String,
    n_inputs: usize,
    n_outputs: usize,
    weights: Vec<f32>, // Flattened weight matrix
    biases: Vec<f32>,
}

impl NeuralPolicy {
    /// Create a new neural policy with random weights
    pub fn new(n_inputs: usize, n_outputs: usize) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        // Simple single-layer network
        let weights: Vec<f32> = (0..n_inputs * n_outputs)
            .map(|_| rng.gen_range(-0.1..0.1))
            .collect();
        let biases: Vec<f32> = vec![0.0; n_outputs];

        Self {
            name: "NeuralPolicy".to_string(),
            n_inputs,
            n_outputs,
            weights,
            biases,
        }
    }

    /// Load from serialized weights
    pub fn from_weights(n_inputs: usize, n_outputs: usize, weights: Vec<f32>, biases: Vec<f32>) -> Result<Self> {
        if weights.len() != n_inputs * n_outputs {
            return Err(crate::error::FxnnError::invalid_parameter(
                format!("Expected {} weights, got {}", n_inputs * n_outputs, weights.len())
            ));
        }
        if biases.len() != n_outputs {
            return Err(crate::error::FxnnError::invalid_parameter(
                format!("Expected {} biases, got {}", n_outputs, biases.len())
            ));
        }

        Ok(Self {
            name: "NeuralPolicy".to_string(),
            n_inputs,
            n_outputs,
            weights,
            biases,
        })
    }

    /// Forward pass through the network
    fn forward(&self, inputs: &[f32]) -> Vec<f32> {
        let mut outputs = self.biases.clone();

        for (o, output) in outputs.iter_mut().enumerate() {
            for (i, &input) in inputs.iter().enumerate() {
                *output += input * self.weights[o * self.n_inputs + i];
            }
            // Tanh activation to bound to [-1, 1]
            *output = output.tanh();
        }

        outputs
    }
}

impl Policy for NeuralPolicy {
    fn name(&self) -> &str {
        &self.name
    }

    fn decide(
        &self,
        sensor_readings: &[SensorReading],
        goal_values: &[f32],
    ) -> Result<PolicyOutput> {
        // Flatten sensor readings into input vector
        let mut inputs: Vec<f32> = Vec::with_capacity(self.n_inputs);

        for reading in sensor_readings {
            inputs.extend(&reading.values);
        }
        inputs.extend(goal_values);

        // Pad or truncate to expected size
        inputs.resize(self.n_inputs, 0.0);

        let outputs = self.forward(&inputs);

        Ok(PolicyOutput {
            action_intensities: outputs,
            confidence: 0.8, // Neural policies have moderate confidence
            rationale: "Neural network inference".to_string(),
            state_update: None,
        })
    }

    fn n_outputs(&self) -> usize {
        self.n_outputs
    }

    fn is_deterministic(&self) -> bool {
        true // This simple network is deterministic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_random_policy() {
        let policy = RandomPolicy::new().with_n_outputs(3);
        let output = policy.decide(&[], &[]).unwrap();

        assert_eq!(output.action_intensities.len(), 3);
        for &v in &output.action_intensities {
            assert!(v >= -1.0 && v <= 1.0);
        }
    }

    #[test]
    fn test_neural_policy() {
        let policy = NeuralPolicy::new(10, 3);
        let readings = vec![SensorReading {
            sensor_id: super::super::sensor::SensorId(0),
            kind: super::super::sensor::SensorKind::Proprioceptive,
            values: vec![0.5; 10],
            timestamp: 0,
            noise_level: 0.0,
        }];

        let output = policy.decide(&readings, &[]).unwrap();
        assert_eq!(output.action_intensities.len(), 3);
    }
}
