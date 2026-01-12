//! Policy network traits and implementations.
//!
//! This module provides the policy network abstraction for agent decision-making.
//! Policies map sensor observations to actuator commands.
//!
//! # Policy Types
//!
//! | Policy | Description | Use Case |
//! |--------|-------------|----------|
//! | [`SimplePolicy`] | Hardcoded rules | Baseline/testing |
//! | [`NeuralPolicy`] | Neural network | Learning agents |
//!
//! # Example
//!
//! ```rust,no_run
//! use fxnn::agency::{SimplePolicy, PolicyNetwork, SensorReading};
//!
//! // Create a simple rule-based policy
//! let policy = SimplePolicy::random();
//!
//! // Get action from observations
//! let observations: Vec<SensorReading> = vec![];
//! let output = policy.forward(&observations);
//! ```

use super::sensor::SensorReading;
use super::actuator::ActuatorCommand;

/// Output from a policy network.
#[derive(Debug, Clone)]
pub struct PolicyOutput {
    /// Commands for each actuator.
    pub commands: Vec<ActuatorCommand>,
    /// Value estimate (for actor-critic methods).
    pub value: f32,
    /// Action log probability (for policy gradient methods).
    pub log_prob: f32,
    /// Entropy of the action distribution.
    pub entropy: f32,
}

impl PolicyOutput {
    /// Create a new policy output with just commands.
    pub fn new(commands: Vec<ActuatorCommand>) -> Self {
        Self {
            commands,
            value: 0.0,
            log_prob: 0.0,
            entropy: 0.0,
        }
    }

    /// Create a policy output with value estimate.
    pub fn with_value(mut self, value: f32) -> Self {
        self.value = value;
        self
    }

    /// Create a policy output with log probability.
    pub fn with_log_prob(mut self, log_prob: f32) -> Self {
        self.log_prob = log_prob;
        self
    }
}

impl Default for PolicyOutput {
    fn default() -> Self {
        Self {
            commands: Vec::new(),
            value: 0.0,
            log_prob: 0.0,
            entropy: 0.0,
        }
    }
}

/// Trait for policy networks.
///
/// A policy maps observations to actions. This trait provides a common
/// interface for both simple rule-based policies and complex neural
/// network policies.
pub trait PolicyNetwork: Send + Sync {
    /// Compute the forward pass of the policy.
    ///
    /// # Arguments
    ///
    /// * `observations` - Sensor readings from all sensors
    ///
    /// # Returns
    ///
    /// Policy output containing actuator commands and metadata.
    fn forward(&self, observations: &[SensorReading]) -> PolicyOutput;

    /// Update the policy with a gradient step.
    ///
    /// # Arguments
    ///
    /// * `observations` - Input observations
    /// * `actions` - Actions that were taken
    /// * `advantages` - Advantage estimates for each action
    /// * `learning_rate` - Step size for the update
    ///
    /// # Returns
    ///
    /// The loss value after the update.
    fn update(
        &mut self,
        observations: &[Vec<SensorReading>],
        actions: &[PolicyOutput],
        advantages: &[f32],
        learning_rate: f32,
    ) -> f32;

    /// Get the number of parameters in the policy.
    fn parameter_count(&self) -> usize;

    /// Get a name/description of the policy.
    fn name(&self) -> &str;

    /// Reset any internal state (e.g., for recurrent policies).
    fn reset(&mut self);

    /// Check if the policy supports training.
    fn is_trainable(&self) -> bool;

    /// Get the policy parameters as a flat vector (for saving/loading).
    fn get_parameters(&self) -> Vec<f32>;

    /// Set the policy parameters from a flat vector.
    fn set_parameters(&mut self, params: &[f32]);
}

// ============================================================================
// Simple Policy (Rule-Based)
// ============================================================================

/// Rule type for simple policies.
#[derive(Debug, Clone)]
pub enum PolicyRule {
    /// Always output the same action.
    Constant(Vec<ActuatorCommand>),
    /// Random action from a set.
    Random(Vec<Vec<ActuatorCommand>>),
    /// Seek the nearest visible entity.
    SeekNearest,
    /// Avoid the nearest visible entity.
    AvoidNearest,
    /// Follow a simple gradient.
    GradientFollow,
    /// Wander randomly.
    Wander,
    /// Custom rule with a function.
    Custom(String),
}

/// Simple rule-based policy.
///
/// Uses predefined rules to map observations to actions.
/// Useful for baselines, testing, and simple behaviors.
#[derive(Debug, Clone)]
pub struct SimplePolicy {
    /// Rule to apply.
    rule: PolicyRule,
    /// Name of the policy.
    name: String,
    /// Internal state (for stateful behaviors like wandering).
    state: SimplePolicyState,
}

#[derive(Debug, Clone, Default)]
struct SimplePolicyState {
    /// Current wander direction.
    wander_direction: [f32; 3],
    /// Steps since last direction change.
    wander_steps: u32,
    /// Random seed state.
    rng_state: u64,
}

impl SimplePolicy {
    /// Create a constant policy that always outputs the same action.
    pub fn constant(commands: Vec<ActuatorCommand>) -> Self {
        Self {
            rule: PolicyRule::Constant(commands),
            name: "ConstantPolicy".to_string(),
            state: SimplePolicyState::default(),
        }
    }

    /// Create a random policy that chooses from a set of actions.
    pub fn random() -> Self {
        Self {
            rule: PolicyRule::Random(vec![]),
            name: "RandomPolicy".to_string(),
            state: SimplePolicyState {
                rng_state: 12345,
                ..Default::default()
            },
        }
    }

    /// Create a seek-nearest policy.
    pub fn seek_nearest() -> Self {
        Self {
            rule: PolicyRule::SeekNearest,
            name: "SeekNearestPolicy".to_string(),
            state: SimplePolicyState::default(),
        }
    }

    /// Create an avoid-nearest policy.
    pub fn avoid_nearest() -> Self {
        Self {
            rule: PolicyRule::AvoidNearest,
            name: "AvoidNearestPolicy".to_string(),
            state: SimplePolicyState::default(),
        }
    }

    /// Create a wander policy.
    pub fn wander() -> Self {
        Self {
            rule: PolicyRule::Wander,
            name: "WanderPolicy".to_string(),
            state: SimplePolicyState {
                wander_direction: [1.0, 0.0, 0.0],
                wander_steps: 0,
                rng_state: 42,
            },
        }
    }

    /// Set a custom name for the policy.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Simple pseudo-random number generator.
    fn next_random(&mut self) -> f32 {
        // Simple xorshift
        let mut x = self.state.rng_state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state.rng_state = x;
        (x as f32) / (u64::MAX as f32)
    }
}

impl PolicyNetwork for SimplePolicy {
    fn forward(&self, observations: &[SensorReading]) -> PolicyOutput {
        use super::actuator::MotorCommand;

        let commands = match &self.rule {
            PolicyRule::Constant(cmds) => cmds.clone(),

            PolicyRule::Random(_) | PolicyRule::Wander => {
                // Generate random force direction
                vec![ActuatorCommand::Motor(MotorCommand::relative_force(
                    1.0, 0.0, 0.0,
                ))]
            }

            PolicyRule::SeekNearest => {
                // Find nearest visible entity and move towards it
                for obs in observations {
                    if let SensorReading::Vision(vision) = obs {
                        if let Some(nearest) = vision.nearest() {
                            // Move towards the nearest entity
                            let dir = nearest.relative_position;
                            let mag = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
                            if mag > 0.1 {
                                return PolicyOutput::new(vec![
                                    ActuatorCommand::Motor(MotorCommand::relative_force(
                                        dir[0] / mag,
                                        dir[1] / mag,
                                        dir[2] / mag,
                                    ))
                                ]);
                            }
                        }
                    }
                }
                vec![ActuatorCommand::None]
            }

            PolicyRule::AvoidNearest => {
                // Find nearest visible entity and move away from it
                for obs in observations {
                    if let SensorReading::Vision(vision) = obs {
                        if let Some(nearest) = vision.nearest() {
                            // Move away from the nearest entity
                            let dir = nearest.relative_position;
                            let mag = (dir[0] * dir[0] + dir[1] * dir[1] + dir[2] * dir[2]).sqrt();
                            if mag > 0.1 {
                                return PolicyOutput::new(vec![
                                    ActuatorCommand::Motor(MotorCommand::relative_force(
                                        -dir[0] / mag,
                                        -dir[1] / mag,
                                        -dir[2] / mag,
                                    ))
                                ]);
                            }
                        }
                    }
                }
                vec![ActuatorCommand::None]
            }

            PolicyRule::GradientFollow => {
                // Would follow some gradient in the environment
                vec![ActuatorCommand::None]
            }

            PolicyRule::Custom(_) => {
                vec![ActuatorCommand::None]
            }
        };

        PolicyOutput::new(commands)
    }

    fn update(
        &mut self,
        _observations: &[Vec<SensorReading>],
        _actions: &[PolicyOutput],
        _advantages: &[f32],
        _learning_rate: f32,
    ) -> f32 {
        // Simple policies don't learn
        0.0
    }

    fn parameter_count(&self) -> usize {
        0
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn reset(&mut self) {
        self.state = SimplePolicyState::default();
    }

    fn is_trainable(&self) -> bool {
        false
    }

    fn get_parameters(&self) -> Vec<f32> {
        Vec::new()
    }

    fn set_parameters(&mut self, _params: &[f32]) {
        // No parameters to set
    }
}

// ============================================================================
// Neural Policy
// ============================================================================

/// Activation function types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Activation {
    /// Rectified Linear Unit.
    ReLU,
    /// Hyperbolic tangent.
    Tanh,
    /// Sigmoid.
    Sigmoid,
    /// Linear (no activation).
    Linear,
    /// Softmax (for output layer).
    Softmax,
}

/// A layer in the neural policy.
#[derive(Debug, Clone)]
pub struct NeuralLayer {
    /// Weight matrix (input_size x output_size).
    pub weights: Vec<f32>,
    /// Bias vector (output_size).
    pub biases: Vec<f32>,
    /// Input dimension.
    pub input_size: usize,
    /// Output dimension.
    pub output_size: usize,
    /// Activation function.
    pub activation: Activation,
}

impl NeuralLayer {
    /// Create a new layer with random initialization.
    pub fn new(input_size: usize, output_size: usize, activation: Activation) -> Self {
        // Xavier initialization
        let scale = (2.0 / (input_size + output_size) as f32).sqrt();
        let mut weights = vec![0.0; input_size * output_size];
        let mut biases = vec![0.0; output_size];

        // Simple pseudo-random initialization
        let mut seed = 42u64;
        for w in &mut weights {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u = (seed as f32) / (u64::MAX as f32);
            *w = (u * 2.0 - 1.0) * scale;
        }
        for b in &mut biases {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let u = (seed as f32) / (u64::MAX as f32);
            *b = (u * 2.0 - 1.0) * scale * 0.1;
        }

        Self {
            weights,
            biases,
            input_size,
            output_size,
            activation,
        }
    }

    /// Compute the forward pass of this layer.
    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        assert_eq!(input.len(), self.input_size);

        let mut output = self.biases.clone();

        // Matrix-vector multiplication
        for j in 0..self.output_size {
            for i in 0..self.input_size {
                output[j] += input[i] * self.weights[i * self.output_size + j];
            }
        }

        // Apply activation
        match self.activation {
            Activation::ReLU => {
                for x in &mut output {
                    *x = x.max(0.0);
                }
            }
            Activation::Tanh => {
                for x in &mut output {
                    *x = x.tanh();
                }
            }
            Activation::Sigmoid => {
                for x in &mut output {
                    *x = 1.0 / (1.0 + (-*x).exp());
                }
            }
            Activation::Softmax => {
                let max_val = output.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
                let sum: f32 = output.iter().map(|x| (x - max_val).exp()).sum();
                for x in &mut output {
                    *x = (*x - max_val).exp() / sum;
                }
            }
            Activation::Linear => {
                // No activation
            }
        }

        output
    }

    /// Get the number of parameters in this layer.
    pub fn parameter_count(&self) -> usize {
        self.weights.len() + self.biases.len()
    }
}

/// Neural network policy.
///
/// Uses a multi-layer perceptron to map observations to actions.
/// Supports training via policy gradient methods.
#[derive(Debug, Clone)]
pub struct NeuralPolicy {
    /// Hidden layers.
    layers: Vec<NeuralLayer>,
    /// Policy head (outputs action parameters).
    policy_head: NeuralLayer,
    /// Value head (outputs value estimate).
    value_head: Option<NeuralLayer>,
    /// Name of the policy.
    name: String,
    /// Input dimension.
    input_size: usize,
    /// Output dimension (number of action parameters).
    output_size: usize,
    /// Whether this policy uses continuous or discrete actions.
    continuous: bool,
}

impl NeuralPolicy {
    /// Create a new neural policy.
    ///
    /// # Arguments
    ///
    /// * `input_size` - Dimension of the observation vector
    /// * `hidden_sizes` - Sizes of hidden layers
    /// * `output_size` - Dimension of the action vector
    /// * `continuous` - Whether actions are continuous or discrete
    ///
    /// # Returns
    ///
    /// A new `NeuralPolicy`.
    pub fn new(
        input_size: usize,
        hidden_sizes: &[usize],
        output_size: usize,
        continuous: bool,
    ) -> Self {
        let mut layers = Vec::new();
        let mut prev_size = input_size;

        // Build hidden layers
        for &size in hidden_sizes {
            layers.push(NeuralLayer::new(prev_size, size, Activation::ReLU));
            prev_size = size;
        }

        // Policy head
        let policy_activation = if continuous {
            Activation::Tanh
        } else {
            Activation::Softmax
        };
        let policy_head = NeuralLayer::new(prev_size, output_size, policy_activation);

        // Value head
        let value_head = Some(NeuralLayer::new(prev_size, 1, Activation::Linear));

        Self {
            layers,
            policy_head,
            value_head,
            name: "NeuralPolicy".to_string(),
            input_size,
            output_size,
            continuous,
        }
    }

    /// Create a simple MLP policy with one hidden layer.
    pub fn simple(input_size: usize, hidden_size: usize, output_size: usize) -> Self {
        Self::new(input_size, &[hidden_size], output_size, true)
    }

    /// Set a custom name (builder pattern).
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Remove the value head (builder pattern).
    pub fn without_value_head(mut self) -> Self {
        self.value_head = None;
        self
    }

    /// Convert observations to a flat input vector.
    fn observations_to_vector(observations: &[SensorReading]) -> Vec<f32> {
        observations
            .iter()
            .flat_map(|r| r.to_vector())
            .collect()
    }

    /// Convert network output to actuator commands.
    fn output_to_commands(&self, output: &[f32]) -> Vec<ActuatorCommand> {
        use super::actuator::MotorCommand;

        // For now, interpret output as motor force components
        if output.len() >= 3 {
            vec![ActuatorCommand::Motor(MotorCommand::force(
                output[0],
                output[1],
                output[2],
            ))]
        } else {
            vec![ActuatorCommand::None]
        }
    }
}

impl PolicyNetwork for NeuralPolicy {
    fn forward(&self, observations: &[SensorReading]) -> PolicyOutput {
        // Convert observations to input vector
        let mut x = Self::observations_to_vector(observations);

        // Pad or truncate to match expected input size
        x.resize(self.input_size, 0.0);

        // Forward through hidden layers
        for layer in &self.layers {
            x = layer.forward(&x);
        }

        // Store hidden state for value computation
        let hidden = x.clone();

        // Policy head
        let action_params = self.policy_head.forward(&hidden);

        // Value head
        let value = if let Some(ref value_head) = self.value_head {
            value_head.forward(&hidden)[0]
        } else {
            0.0
        };

        // Convert to commands
        let commands = self.output_to_commands(&action_params);

        PolicyOutput {
            commands,
            value,
            log_prob: 0.0, // Would be computed for training
            entropy: 0.0,  // Would be computed for training
        }
    }

    fn update(
        &mut self,
        _observations: &[Vec<SensorReading>],
        _actions: &[PolicyOutput],
        _advantages: &[f32],
        _learning_rate: f32,
    ) -> f32 {
        // TODO: Implement backpropagation for policy gradient
        // This would typically use PPO, A2C, or similar algorithm
        0.0
    }

    fn parameter_count(&self) -> usize {
        let mut count = 0;
        for layer in &self.layers {
            count += layer.parameter_count();
        }
        count += self.policy_head.parameter_count();
        if let Some(ref vh) = self.value_head {
            count += vh.parameter_count();
        }
        count
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn reset(&mut self) {
        // No internal state to reset for MLP
    }

    fn is_trainable(&self) -> bool {
        true
    }

    fn get_parameters(&self) -> Vec<f32> {
        let mut params = Vec::with_capacity(self.parameter_count());

        for layer in &self.layers {
            params.extend_from_slice(&layer.weights);
            params.extend_from_slice(&layer.biases);
        }
        params.extend_from_slice(&self.policy_head.weights);
        params.extend_from_slice(&self.policy_head.biases);

        if let Some(ref vh) = self.value_head {
            params.extend_from_slice(&vh.weights);
            params.extend_from_slice(&vh.biases);
        }

        params
    }

    fn set_parameters(&mut self, params: &[f32]) {
        let mut offset = 0;

        for layer in &mut self.layers {
            let weight_count = layer.weights.len();
            layer.weights.copy_from_slice(&params[offset..offset + weight_count]);
            offset += weight_count;

            let bias_count = layer.biases.len();
            layer.biases.copy_from_slice(&params[offset..offset + bias_count]);
            offset += bias_count;
        }

        let weight_count = self.policy_head.weights.len();
        self.policy_head.weights.copy_from_slice(&params[offset..offset + weight_count]);
        offset += weight_count;

        let bias_count = self.policy_head.biases.len();
        self.policy_head.biases.copy_from_slice(&params[offset..offset + bias_count]);
        offset += bias_count;

        if let Some(ref mut vh) = self.value_head {
            let weight_count = vh.weights.len();
            vh.weights.copy_from_slice(&params[offset..offset + weight_count]);
            offset += weight_count;

            let bias_count = vh.biases.len();
            vh.biases.copy_from_slice(&params[offset..offset + bias_count]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_policy() {
        let policy = SimplePolicy::wander();
        assert_eq!(policy.name(), "WanderPolicy");
        assert!(!policy.is_trainable());

        let output = policy.forward(&[]);
        assert_eq!(output.commands.len(), 1);
    }

    #[test]
    fn test_neural_policy_creation() {
        let policy = NeuralPolicy::new(10, &[32, 16], 4, true);
        assert!(policy.parameter_count() > 0);
        assert!(policy.is_trainable());
    }

    #[test]
    fn test_neural_layer() {
        let layer = NeuralLayer::new(4, 2, Activation::ReLU);
        let input = vec![1.0, 0.5, -0.5, 0.0];
        let output = layer.forward(&input);

        assert_eq!(output.len(), 2);
        // ReLU should make all outputs non-negative
        for x in &output {
            assert!(*x >= 0.0 || (*x - 0.0).abs() < 1e-6);
        }
    }

    #[test]
    fn test_parameter_save_load() {
        let mut policy = NeuralPolicy::simple(4, 8, 2);
        let original_params = policy.get_parameters();

        // Modify parameters
        let mut modified = original_params.clone();
        for p in &mut modified {
            *p *= 2.0;
        }
        policy.set_parameters(&modified);

        let loaded = policy.get_parameters();
        for (m, l) in modified.iter().zip(loaded.iter()) {
            assert!((m - l).abs() < 1e-6);
        }
    }
}
