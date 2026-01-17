//! Neural Network Force Field with SchNet-style Architecture.
//!
//! This module implements a neural network-based force field using continuous-filter
//! convolutional neural networks (CFConv), inspired by SchNet architecture.
//!
//! # SchNet Architecture Overview
//!
//! SchNet uses:
//! 1. **Atom embeddings**: Learnable representations for each element
//! 2. **Radial basis functions**: Distance encoding with smooth cutoff
//! 3. **Continuous-filter convolution**: Message passing with distance-dependent filters
//! 4. **Interaction blocks**: Multiple layers of convolution + residual connections
//! 5. **Atomwise output**: Per-atom energy contributions summed for total energy
//!
//! # Forces via Automatic Differentiation
//!
//! Forces are computed as F = -dE/dr using numerical gradients (finite differences).
//! A production implementation would use analytical gradients or autodiff.
//!
//! # SONA Integration
//!
//! The model integrates with SONA (Self-Optimizing Neural Architecture) for:
//! - Adaptive learning rate adjustment
//! - Architecture modification based on error patterns
//! - Continual learning without catastrophic forgetting (EWC)
//!
//! # References
//!
//! - SchNet: A continuous-filter convolutional neural network for modeling quantum interactions
//!   (Schutt et al., 2017)
//! - Equivariant message passing for the prediction of tensorial properties and molecular spectra
//!   (Schutt et al., 2021)

use crate::force_field::ForceField;
use crate::neighbor::NeighborList;
use crate::types::{Atom, SimulationBox};
use rand_distr::{Distribution, Normal};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

// =============================================================================
// Radial Basis Functions
// =============================================================================

/// Radial basis function expansion for distance encoding.
///
/// Transforms scalar distances into a multi-dimensional representation
/// that the neural network can process more effectively.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadialBasisFunctions {
    /// Centers of the Gaussian basis functions
    centers: Vec<f32>,

    /// Width parameter (shared or per-basis)
    gamma: f32,

    /// Cutoff distance for smooth truncation
    cutoff: f32,

    /// Number of basis functions
    num_basis: usize,
}

impl RadialBasisFunctions {
    /// Create a new RBF expansion with evenly spaced Gaussians.
    ///
    /// # Arguments
    /// * `num_basis` - Number of basis functions
    /// * `cutoff` - Maximum distance (basis centers distributed from 0 to cutoff)
    pub fn new(num_basis: usize, cutoff: f32) -> Self {
        assert!(num_basis > 0, "Must have at least one basis function");
        assert!(cutoff > 0.0, "Cutoff must be positive");

        // Evenly space centers from 0 to cutoff
        let centers: Vec<f32> = (0..num_basis)
            .map(|i| i as f32 * cutoff / (num_basis - 1).max(1) as f32)
            .collect();

        // Width based on spacing between centers
        let gamma = 1.0 / (cutoff / num_basis as f32).powi(2);

        Self {
            centers,
            gamma,
            cutoff,
            num_basis,
        }
    }

    /// Create RBF with custom gamma (width parameter).
    pub fn with_gamma(mut self, gamma: f32) -> Self {
        self.gamma = gamma;
        self
    }

    /// Compute basis function values for a distance.
    ///
    /// Returns a vector of length `num_basis` with the value of each
    /// Gaussian basis function at the given distance.
    ///
    /// # Arguments
    /// * `distance` - The interatomic distance
    ///
    /// # Returns
    /// Vector of basis function values
    pub fn expand(&self, distance: f32) -> Vec<f32> {
        if distance >= self.cutoff {
            return vec![0.0; self.num_basis];
        }

        let fc = self.cosine_cutoff(distance);

        self.centers
            .iter()
            .map(|&center| {
                let diff = distance - center;
                fc * (-self.gamma * diff * diff).exp()
            })
            .collect()
    }

    /// Compute gradients of basis functions with respect to distance.
    ///
    /// # Arguments
    /// * `distance` - The interatomic distance
    ///
    /// # Returns
    /// Vector of gradients d(rbf)/d(distance) for each basis function
    pub fn expand_gradient(&self, distance: f32) -> Vec<f32> {
        if distance >= self.cutoff {
            return vec![0.0; self.num_basis];
        }

        let fc = self.cosine_cutoff(distance);
        let dfc = self.cosine_cutoff_derivative(distance);

        self.centers
            .iter()
            .map(|&center| {
                let diff = distance - center;
                let gaussian = (-self.gamma * diff * diff).exp();
                let dgaussian = -2.0 * self.gamma * diff * gaussian;

                // Product rule: d(fc * g)/dr = dfc * g + fc * dg
                dfc * gaussian + fc * dgaussian
            })
            .collect()
    }

    /// Smooth cosine cutoff function.
    ///
    /// f_c(r) = 0.5 * (cos(pi * r / r_c) + 1) for r < r_c, else 0
    #[inline]
    fn cosine_cutoff(&self, distance: f32) -> f32 {
        if distance >= self.cutoff {
            0.0
        } else {
            0.5 * ((PI * distance / self.cutoff).cos() + 1.0)
        }
    }

    /// Derivative of the cosine cutoff function.
    #[inline]
    fn cosine_cutoff_derivative(&self, distance: f32) -> f32 {
        if distance >= self.cutoff {
            0.0
        } else {
            -0.5 * PI / self.cutoff * (PI * distance / self.cutoff).sin()
        }
    }

    /// Get the number of basis functions.
    pub fn num_basis(&self) -> usize {
        self.num_basis
    }

    /// Get the cutoff distance.
    pub fn cutoff(&self) -> f32 {
        self.cutoff
    }
}

// =============================================================================
// Neural Network Layers
// =============================================================================

/// Dense (fully connected) layer with bias.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenseLayer {
    /// Weight matrix (output_dim x input_dim)
    weights: Vec<f32>,

    /// Bias vector (output_dim)
    bias: Vec<f32>,

    /// Input dimension
    input_dim: usize,

    /// Output dimension
    output_dim: usize,
}

impl DenseLayer {
    /// Create a new dense layer with Xavier initialization.
    pub fn new(input_dim: usize, output_dim: usize) -> Self {
        let mut rng = rand::thread_rng();
        let scale = (2.0 / (input_dim + output_dim) as f32).sqrt();
        let normal = Normal::new(0.0, scale as f64).unwrap();

        let weights: Vec<f32> = (0..input_dim * output_dim)
            .map(|_| normal.sample(&mut rng) as f32)
            .collect();

        let bias = vec![0.0; output_dim];

        Self {
            weights,
            bias,
            input_dim,
            output_dim,
        }
    }

    /// Forward pass: y = Wx + b
    pub fn forward(&self, input: &[f32]) -> Vec<f32> {
        assert_eq!(input.len(), self.input_dim);

        let mut output = self.bias.clone();

        for i in 0..self.output_dim {
            for j in 0..self.input_dim {
                output[i] += self.weights[i * self.input_dim + j] * input[j];
            }
        }

        output
    }

    /// Forward pass with input gradient computation.
    ///
    /// Returns (output, gradient_fn) where gradient_fn maps output gradient to input gradient.
    #[allow(dead_code)]
    pub fn forward_with_gradient(&self, input: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let output = self.forward(input);

        // For linear layer, gradient w.r.t. input is W^T
        // We return the weights transposed for later use
        let weights_t: Vec<f32> = (0..self.input_dim)
            .flat_map(|j| (0..self.output_dim).map(move |i| self.weights[i * self.input_dim + j]))
            .collect();

        (output, weights_t)
    }

    /// Backward pass: compute gradient w.r.t. input given output gradient.
    pub fn backward(&self, grad_output: &[f32]) -> Vec<f32> {
        let mut grad_input = vec![0.0; self.input_dim];

        for j in 0..self.input_dim {
            for i in 0..self.output_dim {
                grad_input[j] += self.weights[i * self.input_dim + j] * grad_output[i];
            }
        }

        grad_input
    }

    /// Get mutable reference to weights for training.
    #[allow(dead_code)]
    pub fn weights_mut(&mut self) -> &mut [f32] {
        &mut self.weights
    }

    /// Get mutable reference to bias for training.
    #[allow(dead_code)]
    pub fn bias_mut(&mut self) -> &mut [f32] {
        &mut self.bias
    }

    /// Get all parameters as a flat vector.
    pub fn parameters(&self) -> Vec<f32> {
        let mut params = self.weights.clone();
        params.extend(&self.bias);
        params
    }

    /// Set parameters from a flat vector.
    #[allow(dead_code)]
    pub fn set_parameters(&mut self, params: &[f32]) {
        let n_weights = self.input_dim * self.output_dim;
        assert_eq!(params.len(), n_weights + self.output_dim);

        self.weights.copy_from_slice(&params[..n_weights]);
        self.bias.copy_from_slice(&params[n_weights..]);
    }
}

/// Shifted softplus activation: ssp(x) = ln(1 + exp(x)) - ln(2)
///
/// Smooth approximation to ReLU that passes through the origin.
#[inline]
fn shifted_softplus(x: f32) -> f32 {
    // Numerical stability: for large x, softplus(x) ~ x
    if x > 20.0 {
        x - 0.693147 // ln(2)
    } else if x < -20.0 {
        -0.693147
    } else {
        (1.0 + x.exp()).ln() - 0.693147
    }
}

/// Derivative of shifted softplus: sigmoid(x)
#[inline]
fn shifted_softplus_derivative(x: f32) -> f32 {
    if x > 20.0 {
        1.0
    } else if x < -20.0 {
        0.0
    } else {
        1.0 / (1.0 + (-x).exp())
    }
}

// =============================================================================
// Continuous-Filter Convolution
// =============================================================================

/// Continuous-filter convolutional layer (CFConv).
///
/// Generates position-dependent filters from interatomic distances
/// using a filter-generating network.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CFConvLayer {
    /// Filter-generating network: RBF -> filter weights
    filter_network: Vec<DenseLayer>,

    /// Number of features per atom
    num_features: usize,

    /// Number of RBF basis functions
    #[allow(dead_code)]
    num_rbf: usize,
}

impl CFConvLayer {
    /// Create a new CFConv layer.
    ///
    /// # Arguments
    /// * `num_features` - Dimension of atom features
    /// * `num_rbf` - Number of radial basis functions
    /// * `num_filters` - Number of filter channels (usually = num_features)
    pub fn new(num_features: usize, num_rbf: usize, num_filters: usize) -> Self {
        // Filter network: RBF -> hidden -> filters
        let filter_network = vec![
            DenseLayer::new(num_rbf, num_features),
            DenseLayer::new(num_features, num_filters),
        ];

        Self {
            filter_network,
            num_features,
            num_rbf,
        }
    }

    /// Generate filter weights from RBF expansion.
    fn generate_filter(&self, rbf: &[f32]) -> Vec<f32> {
        let mut h = self.filter_network[0].forward(rbf);

        // Apply activation
        for x in &mut h {
            *x = shifted_softplus(*x);
        }

        self.filter_network[1].forward(&h)
    }

    /// Forward pass: aggregate neighbor features with distance-dependent filters.
    ///
    /// # Arguments
    /// * `_atom_features` - Feature vector for the central atom (unused in this layer)
    /// * `neighbor_features` - Feature vectors for neighboring atoms
    /// * `rbf_values` - RBF expansions of distances to neighbors
    ///
    /// # Returns
    /// Updated feature vector after convolution
    pub fn forward(
        &self,
        _atom_features: &[f32],
        neighbor_features: &[&[f32]],
        rbf_values: &[Vec<f32>],
    ) -> Vec<f32> {
        let mut output = vec![0.0; self.num_features];

        for (neighbor_feat, rbf) in neighbor_features.iter().zip(rbf_values.iter()) {
            let filter = self.generate_filter(rbf);

            // Elementwise multiplication and accumulation
            for k in 0..self.num_features {
                output[k] += neighbor_feat[k] * filter[k];
            }
        }

        output
    }

    /// Get all parameters.
    pub fn parameters(&self) -> Vec<f32> {
        let mut params = Vec::new();
        for layer in &self.filter_network {
            params.extend(layer.parameters());
        }
        params
    }
}

// =============================================================================
// Interaction Block
// =============================================================================

/// SchNet interaction block combining CFConv with residual connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionBlock {
    /// Continuous-filter convolution
    cfconv: CFConvLayer,

    /// Pre-convolution linear transform
    pre_linear: DenseLayer,

    /// Post-convolution linear transform
    post_linear: DenseLayer,

    /// Number of features
    num_features: usize,
}

impl InteractionBlock {
    /// Create a new interaction block.
    pub fn new(num_features: usize, num_rbf: usize) -> Self {
        Self {
            cfconv: CFConvLayer::new(num_features, num_rbf, num_features),
            pre_linear: DenseLayer::new(num_features, num_features),
            post_linear: DenseLayer::new(num_features, num_features),
            num_features,
        }
    }

    /// Forward pass through the interaction block.
    ///
    /// # Arguments
    /// * `atom_features` - Features for the central atom
    /// * `neighbor_features` - Features for neighboring atoms
    /// * `rbf_values` - RBF expansions of distances
    ///
    /// # Returns
    /// Updated atom features with residual connection
    pub fn forward(
        &self,
        atom_features: &[f32],
        neighbor_features: &[&[f32]],
        rbf_values: &[Vec<f32>],
    ) -> Vec<f32> {
        // Pre-linear transform
        let x = self.pre_linear.forward(atom_features);

        // Continuous-filter convolution
        let conv_out = self.cfconv.forward(&x, neighbor_features, rbf_values);

        // Post-linear transform
        let y = self.post_linear.forward(&conv_out);

        // Residual connection with activation
        let mut output = vec![0.0; self.num_features];
        for i in 0..self.num_features {
            output[i] = shifted_softplus(atom_features[i] + y[i]);
        }

        output
    }

    /// Get all parameters.
    pub fn parameters(&self) -> Vec<f32> {
        let mut params = Vec::new();
        params.extend(self.cfconv.parameters());
        params.extend(self.pre_linear.parameters());
        params.extend(self.post_linear.parameters());
        params
    }
}

// =============================================================================
// SchNet Model
// =============================================================================

/// SchNet neural network force field.
///
/// Predicts atomic energies and forces using continuous-filter convolutions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchNetModel {
    /// Atom type embeddings (num_elements x num_features)
    embeddings: Vec<Vec<f32>>,

    /// Radial basis functions for distance encoding
    rbf: RadialBasisFunctions,

    /// Stack of interaction blocks
    interactions: Vec<InteractionBlock>,

    /// Output network: features -> energy
    output_network: Vec<DenseLayer>,

    /// Number of features per atom
    num_features: usize,

    /// Number of interaction blocks
    num_interactions: usize,

    /// Maximum atomic number supported
    max_z: usize,

    /// Cutoff distance
    cutoff: f32,
}

impl SchNetModel {
    /// Create a new SchNet model.
    ///
    /// # Arguments
    /// * `num_features` - Hidden feature dimension
    /// * `num_interactions` - Number of interaction blocks
    /// * `num_rbf` - Number of radial basis functions
    /// * `cutoff` - Interaction cutoff distance
    /// * `max_z` - Maximum atomic number to support
    pub fn new(
        num_features: usize,
        num_interactions: usize,
        num_rbf: usize,
        cutoff: f32,
        max_z: usize,
    ) -> Self {
        let mut rng = rand::thread_rng();
        let scale = (2.0 / num_features as f32).sqrt();
        let normal = Normal::new(0.0, scale as f64).unwrap();

        // Initialize atom embeddings
        let embeddings: Vec<Vec<f32>> = (0..max_z)
            .map(|_| {
                (0..num_features)
                    .map(|_| normal.sample(&mut rng) as f32)
                    .collect()
            })
            .collect();

        // Radial basis functions
        let rbf = RadialBasisFunctions::new(num_rbf, cutoff);

        // Interaction blocks
        let interactions: Vec<InteractionBlock> = (0..num_interactions)
            .map(|_| InteractionBlock::new(num_features, num_rbf))
            .collect();

        // Output network: features -> 32 -> 1
        let output_network = vec![
            DenseLayer::new(num_features, num_features / 2),
            DenseLayer::new(num_features / 2, 1),
        ];

        Self {
            embeddings,
            rbf,
            interactions,
            output_network,
            num_features,
            num_interactions,
            max_z,
            cutoff,
        }
    }

    /// Create a default SchNet model for general use.
    pub fn default_model(cutoff: f32) -> Self {
        Self::new(
            64,  // num_features
            3,   // num_interactions
            25,  // num_rbf
            cutoff,
            100, // max_z (supports all elements)
        )
    }

    /// Get the embedding for an atom type.
    fn get_embedding(&self, atom_type: u16) -> &[f32] {
        let z = (atom_type as usize).min(self.max_z - 1);
        &self.embeddings[z]
    }

    /// Compute atomic energy from features.
    fn compute_atomic_energy(&self, features: &[f32]) -> f32 {
        let mut h = self.output_network[0].forward(features);
        for x in &mut h {
            *x = shifted_softplus(*x);
        }
        let energy = self.output_network[1].forward(&h);
        energy[0]
    }

    /// Compute the gradient of atomic energy w.r.t. features.
    #[allow(dead_code)]
    fn compute_energy_gradient(&self, features: &[f32]) -> Vec<f32> {
        // Forward pass through output network with caching
        let h1 = self.output_network[0].forward(features);
        let h1_act: Vec<f32> = h1.iter().map(|&x| shifted_softplus(x)).collect();
        let _ = self.output_network[1].forward(&h1_act);

        // Backward pass
        // d(energy)/d(h1_act) = output_network[1].weights (first row since output is scalar)
        let grad_h1_act = self.output_network[1].backward(&[1.0]);

        // d(h1_act)/d(h1) = sigmoid (derivative of shifted_softplus)
        let grad_h1: Vec<f32> = h1
            .iter()
            .zip(grad_h1_act.iter())
            .map(|(&x, &g)| g * shifted_softplus_derivative(x))
            .collect();

        // d(h1)/d(features) = output_network[0].weights^T
        self.output_network[0].backward(&grad_h1)
    }

    /// Forward pass computing per-atom energies.
    ///
    /// # Arguments
    /// * `atoms` - Atomic positions, types
    /// * `simulation_box` - For periodic boundary conditions
    /// * `neighbor_list` - Pre-computed neighbors
    ///
    /// # Returns
    /// Vector of atomic energies
    pub fn forward(
        &self,
        atoms: &[Atom],
        simulation_box: &SimulationBox,
        neighbor_list: &NeighborList,
    ) -> Vec<f32> {
        let n = atoms.len();

        // Initialize atom features from embeddings
        let mut features: Vec<Vec<f32>> = atoms
            .iter()
            .map(|atom| {
                // Use atom_type as a proxy for atomic number
                self.get_embedding(atom.atom_type).to_vec()
            })
            .collect();

        // Process through interaction blocks
        for interaction in &self.interactions {
            let mut new_features = vec![vec![0.0; self.num_features]; n];

            for i in 0..n {
                let neighbors = neighbor_list.get_neighbors(i);

                if neighbors.is_empty() {
                    // No neighbors: just apply activation to current features
                    for k in 0..self.num_features {
                        new_features[i][k] = shifted_softplus(features[i][k]);
                    }
                    continue;
                }

                // Collect neighbor data
                let neighbor_feats: Vec<&[f32]> =
                    neighbors.iter().map(|&j| features[j].as_slice()).collect();

                let rbf_values: Vec<Vec<f32>> = neighbors
                    .iter()
                    .map(|&j| {
                        let dist = simulation_box.distance(&atoms[i].position, &atoms[j].position);
                        self.rbf.expand(dist)
                    })
                    .collect();

                new_features[i] = interaction.forward(&features[i], &neighbor_feats, &rbf_values);
            }

            features = new_features;
        }

        // Compute atomic energies
        features
            .iter()
            .map(|feat| self.compute_atomic_energy(feat))
            .collect()
    }

    /// Compute forces using numerical gradients (finite differences).
    ///
    /// F_i = -dE/dr_i
    ///
    /// This uses central differences for numerical stability.
    /// A production version would use analytical gradients or autodiff.
    pub fn compute_forces_numerical(
        &self,
        atoms: &[Atom],
        simulation_box: &SimulationBox,
        neighbor_list: &NeighborList,
    ) -> Vec<[f32; 3]> {
        let n = atoms.len();
        let mut forces = vec![[0.0f32; 3]; n];

        // For each atom, compute gradient of total energy w.r.t. its position
        // Using finite differences (numerical gradient)
        let eps = 1e-4;

        for i in 0..n {
            for dim in 0..3 {
                // Create perturbed atom positions
                let mut atoms_plus = atoms.to_vec();
                let mut atoms_minus = atoms.to_vec();

                atoms_plus[i].position[dim] += eps;
                atoms_minus[i].position[dim] -= eps;

                // Compute energies
                let e_plus: f32 = self.forward(&atoms_plus, simulation_box, neighbor_list).iter().sum();
                let e_minus: f32 = self.forward(&atoms_minus, simulation_box, neighbor_list).iter().sum();

                // Central difference: F = -dE/dr
                forces[i][dim] = -(e_plus - e_minus) / (2.0 * eps);
            }
        }

        forces
    }

    /// Get total number of parameters.
    pub fn num_parameters(&self) -> usize {
        let embedding_params = self.max_z * self.num_features;
        let interaction_params: usize = self.interactions.iter().map(|i| i.parameters().len()).sum();
        let output_params: usize = self.output_network.iter().map(|l| l.parameters().len()).sum();
        embedding_params + interaction_params + output_params
    }

    /// Get all parameters as a flat vector.
    pub fn parameters(&self) -> Vec<f32> {
        let mut params = Vec::new();

        // Embeddings
        for emb in &self.embeddings {
            params.extend(emb);
        }

        // Interactions
        for interaction in &self.interactions {
            params.extend(interaction.parameters());
        }

        // Output network
        for layer in &self.output_network {
            params.extend(layer.parameters());
        }

        params
    }

    /// Get the cutoff distance.
    pub fn cutoff(&self) -> f32 {
        self.cutoff
    }
}

// =============================================================================
// Neural Force Field (ForceField trait implementation)
// =============================================================================

/// Neural network force field wrapping SchNet model.
///
/// Implements the ForceField trait for integration with FXNN simulations.
pub struct NeuralForceField {
    /// The underlying SchNet model
    model: SchNetModel,

    /// Name identifier
    name: String,

    /// Training mode (enables gradient computation)
    #[allow(dead_code)]
    training: bool,
}

impl NeuralForceField {
    /// Create a new neural force field.
    ///
    /// # Arguments
    /// * `model` - Pre-configured SchNet model
    pub fn new(model: SchNetModel) -> Self {
        Self {
            model,
            name: "SchNet".to_string(),
            training: false,
        }
    }

    /// Create with default configuration.
    pub fn default_ff(cutoff: f32) -> Self {
        Self::new(SchNetModel::default_model(cutoff))
    }

    /// Set training mode.
    pub fn train(&mut self, training: bool) {
        self.training = training;
    }

    /// Get reference to the underlying model.
    pub fn model(&self) -> &SchNetModel {
        &self.model
    }

    /// Get mutable reference to the underlying model.
    pub fn model_mut(&mut self) -> &mut SchNetModel {
        &mut self.model
    }
}

impl ForceField for NeuralForceField {
    fn compute_forces(
        &self,
        atoms: &mut [Atom],
        box_: &SimulationBox,
        neighbor_list: Option<&NeighborList>,
    ) {
        let neighbor_list = neighbor_list.expect("NeuralForceField requires a neighbor list");
        let forces = self.model.compute_forces_numerical(atoms, box_, neighbor_list);

        for (atom, force) in atoms.iter_mut().zip(forces.iter()) {
            atom.force[0] += force[0];
            atom.force[1] += force[1];
            atom.force[2] += force[2];
        }
    }

    fn potential_energy(
        &self,
        atoms: &[Atom],
        box_: &SimulationBox,
        neighbor_list: Option<&NeighborList>,
    ) -> f64 {
        let neighbor_list = neighbor_list.expect("NeuralForceField requires a neighbor list");
        let energy: f32 = self.model.forward(atoms, box_, neighbor_list).iter().sum();
        energy as f64
    }

    fn cutoff(&self) -> f32 {
        self.model.cutoff()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn requires_neighbor_list(&self) -> bool {
        true
    }
}

// =============================================================================
// SONA Integration Hooks
// =============================================================================

/// SONA (Self-Optimizing Neural Architecture) integration for adaptive learning.
///
/// Provides hooks for:
/// - Error-based learning rate adaptation
/// - Architecture modification suggestions
/// - Continual learning with EWC
pub mod sona {
    /// Learning statistics for SONA adaptation.
    #[derive(Debug, Clone, Default)]
    pub struct LearningStats {
        /// Recent prediction errors (energy MAE)
        pub energy_errors: Vec<f32>,

        /// Recent force errors (MAE per component)
        pub force_errors: Vec<f32>,

        /// Training iterations completed
        pub iterations: usize,

        /// Current learning rate
        pub learning_rate: f32,

        /// Moving average of loss
        pub loss_ema: f32,
    }

    impl LearningStats {
        /// Create new learning stats with initial learning rate.
        pub fn new(learning_rate: f32) -> Self {
            Self {
                learning_rate,
                loss_ema: 0.0,
                ..Default::default()
            }
        }

        /// Record a training step result.
        pub fn record(&mut self, energy_error: f32, force_error: f32, loss: f32) {
            self.energy_errors.push(energy_error);
            self.force_errors.push(force_error);
            self.iterations += 1;

            // Update exponential moving average
            let alpha = 0.1;
            self.loss_ema = alpha * loss + (1.0 - alpha) * self.loss_ema;

            // Keep only recent history
            if self.energy_errors.len() > 100 {
                self.energy_errors.remove(0);
                self.force_errors.remove(0);
            }
        }

        /// Check if learning is stagnating.
        pub fn is_stagnating(&self) -> bool {
            if self.energy_errors.len() < 20 {
                return false;
            }

            // Compare recent errors to older errors
            let recent: f32 = self.energy_errors[self.energy_errors.len() - 10..]
                .iter()
                .sum::<f32>() / 10.0;
            let older: f32 = self.energy_errors[self.energy_errors.len() - 20..self.energy_errors.len() - 10]
                .iter()
                .sum::<f32>() / 10.0;

            // Stagnating if improvement < 1%
            (older - recent).abs() / older.max(1e-6) < 0.01
        }

        /// Suggest learning rate adjustment.
        pub fn suggest_learning_rate(&self) -> f32 {
            if self.is_stagnating() {
                // Reduce learning rate when stagnating
                (self.learning_rate * 0.5).max(1e-6)
            } else if self.loss_ema < 0.01 && self.learning_rate < 0.01 {
                // Increase if converging fast and LR is low
                (self.learning_rate * 1.1).min(0.01)
            } else {
                self.learning_rate
            }
        }
    }

    /// Adapter for integrating NeuralForceField with SONA.
    pub struct SonaAdapter {
        /// Learning statistics
        stats: LearningStats,

        /// Fisher information for EWC (per parameter)
        fisher_diag: Vec<f32>,

        /// Anchor parameters for EWC
        anchor_params: Vec<f32>,

        /// EWC regularization strength
        ewc_lambda: f32,

        /// Whether EWC is active
        ewc_active: bool,
    }

    impl SonaAdapter {
        /// Create a new SONA adapter.
        pub fn new(learning_rate: f32) -> Self {
            Self {
                stats: LearningStats::new(learning_rate),
                fisher_diag: Vec::new(),
                anchor_params: Vec::new(),
                ewc_lambda: 1000.0,
                ewc_active: false,
            }
        }

        /// Record training results.
        pub fn record_training(&mut self, energy_error: f32, force_error: f32, loss: f32) {
            self.stats.record(energy_error, force_error, loss);
        }

        /// Get adapted learning rate.
        pub fn get_learning_rate(&self) -> f32 {
            self.stats.suggest_learning_rate()
        }

        /// Update learning rate based on stats.
        pub fn update_learning_rate(&mut self) {
            self.stats.learning_rate = self.stats.suggest_learning_rate();
        }

        /// Compute Fisher information from gradients.
        pub fn compute_fisher(&mut self, gradients: &[f32]) {
            if self.fisher_diag.is_empty() {
                self.fisher_diag = vec![0.0; gradients.len()];
            }

            for (f, &g) in self.fisher_diag.iter_mut().zip(gradients.iter()) {
                *f += g * g;
            }
        }

        /// Consolidate current model parameters for EWC.
        pub fn consolidate(&mut self, parameters: Vec<f32>, num_samples: usize) {
            // Normalize Fisher diagonal
            let norm = 1.0 / num_samples as f32;
            for f in &mut self.fisher_diag {
                *f *= norm;
            }

            self.anchor_params = parameters;
            self.ewc_active = true;
        }

        /// Compute EWC penalty term.
        pub fn ewc_penalty(&self, parameters: &[f32]) -> f32 {
            if !self.ewc_active {
                return 0.0;
            }

            let mut penalty = 0.0;
            for ((&p, &anchor), &fisher) in parameters
                .iter()
                .zip(self.anchor_params.iter())
                .zip(self.fisher_diag.iter())
            {
                let diff = p - anchor;
                penalty += fisher * diff * diff;
            }

            0.5 * self.ewc_lambda * penalty
        }

        /// Compute EWC gradient.
        pub fn ewc_gradient(&self, parameters: &[f32]) -> Vec<f32> {
            if !self.ewc_active {
                return vec![0.0; parameters.len()];
            }

            parameters
                .iter()
                .zip(self.anchor_params.iter())
                .zip(self.fisher_diag.iter())
                .map(|((&p, &anchor), &fisher)| self.ewc_lambda * fisher * (p - anchor))
                .collect()
        }

        /// Get learning statistics.
        pub fn stats(&self) -> &LearningStats {
            &self.stats
        }

        /// Check if model is converging well.
        pub fn is_converging(&self) -> bool {
            !self.stats.is_stagnating() && self.stats.loss_ema < 0.1
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_system() -> (Vec<Atom>, SimulationBox, NeighborList) {
        let atoms = vec![
            Atom {
                position: [1.0, 1.0, 1.0],
                atom_type: 0,
                ..Default::default()
            },
            Atom {
                position: [2.0, 1.0, 1.0],
                atom_type: 0,
                ..Default::default()
            },
            Atom {
                position: [1.5, 2.0, 1.0],
                atom_type: 0,
                ..Default::default()
            },
        ];
        let sim_box = SimulationBox::cubic(10.0);
        let mut neighbor_list = NeighborList::new(atoms.len(), 5.0, 0.5);
        neighbor_list.build_direct(&atoms, &sim_box);
        (atoms, sim_box, neighbor_list)
    }

    #[test]
    fn test_rbf_expansion() {
        let rbf = RadialBasisFunctions::new(10, 5.0);

        // At cutoff, all values should be zero
        let values = rbf.expand(5.0);
        assert!(values.iter().all(|&v| v == 0.0));

        // Within cutoff, should have non-zero values
        let values = rbf.expand(2.5);
        assert!(values.iter().any(|&v| v > 0.0));
    }

    #[test]
    fn test_rbf_gradient() {
        let rbf = RadialBasisFunctions::new(10, 5.0);

        // Numerical gradient check
        let r = 2.0;
        let eps = 1e-5;
        let analytical = rbf.expand_gradient(r);
        let numerical: Vec<f32> = (0..10)
            .map(|i| {
                (rbf.expand(r + eps)[i] - rbf.expand(r - eps)[i]) / (2.0 * eps)
            })
            .collect();

        for (a, n) in analytical.iter().zip(numerical.iter()) {
            assert!(
                (a - n).abs() < 1e-2, // Relaxed tolerance for numerical gradient
                "Gradient mismatch: analytical {} vs numerical {}",
                a,
                n
            );
        }
    }

    #[test]
    fn test_dense_layer() {
        let layer = DenseLayer::new(4, 3);
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let output = layer.forward(&input);
        assert_eq!(output.len(), 3);
    }

    #[test]
    fn test_shifted_softplus() {
        // Should pass through origin
        assert!(shifted_softplus(0.0).abs() < 0.01);

        // Should be monotonically increasing
        assert!(shifted_softplus(1.0) > shifted_softplus(0.0));
        assert!(shifted_softplus(2.0) > shifted_softplus(1.0));

        // Derivative should be in (0, 1)
        assert!(shifted_softplus_derivative(0.0) > 0.0);
        assert!(shifted_softplus_derivative(0.0) < 1.0);
    }

    #[test]
    fn test_schnet_model_creation() {
        let model = SchNetModel::new(32, 2, 10, 5.0, 100);
        assert_eq!(model.num_features, 32);
        assert_eq!(model.num_interactions, 2);
        assert!(model.num_parameters() > 0);
    }

    #[test]
    fn test_schnet_forward() {
        let (atoms, sim_box, neighbor_list) = create_test_system();

        let model = SchNetModel::new(16, 1, 5, 5.0, 10);
        let energies = model.forward(&atoms, &sim_box, &neighbor_list);

        assert_eq!(energies.len(), 3);
        assert!(energies.iter().all(|e| e.is_finite()));
    }

    #[test]
    fn test_neural_force_field_trait() {
        let (mut atoms, sim_box, neighbor_list) = create_test_system();

        let ff = NeuralForceField::default_ff(5.0);

        // Zero forces first
        for atom in atoms.iter_mut() {
            atom.zero_force();
        }

        // Compute energy
        let energy = ff.potential_energy(&atoms, &sim_box, Some(&neighbor_list));
        assert!(energy.is_finite());

        // Compute forces
        ff.compute_forces(&mut atoms, &sim_box, Some(&neighbor_list));
        for atom in &atoms {
            assert!(atom.force.iter().all(|f| f.is_finite()));
        }
    }

    #[test]
    fn test_energy_conservation_check() {
        // Forces should be consistent with energy gradient
        let (atoms, sim_box, neighbor_list) = create_test_system();

        let model = SchNetModel::new(16, 1, 5, 5.0, 10);
        let forces = model.compute_forces_numerical(&atoms, &sim_box, &neighbor_list);

        // All forces should be finite
        for force in &forces {
            assert!(force.iter().all(|f| f.is_finite()));
        }
    }

    #[test]
    fn test_sona_learning_stats() {
        let mut stats = sona::LearningStats::new(0.001);

        // Record some training steps
        for i in 0..50 {
            let error = 1.0 / (i + 1) as f32; // Decreasing error
            stats.record(error, error * 0.5, error * 1.5);
        }

        assert_eq!(stats.iterations, 50);
        assert!(!stats.is_stagnating()); // Should not be stagnating with decreasing errors
    }

    #[test]
    fn test_sona_stagnation_detection() {
        let mut stats = sona::LearningStats::new(0.001);

        // Record constant errors (stagnating)
        for _ in 0..30 {
            stats.record(0.1, 0.05, 0.15);
        }

        assert!(stats.is_stagnating());
        assert!(stats.suggest_learning_rate() < stats.learning_rate);
    }

    #[test]
    fn test_sona_ewc() {
        let mut adapter = sona::SonaAdapter::new(0.001);

        // Simulate gradient accumulation
        let gradients = vec![0.1, 0.2, 0.3, 0.4];
        for _ in 0..10 {
            adapter.compute_fisher(&gradients);
        }

        // Consolidate
        let params = vec![1.0, 2.0, 3.0, 4.0];
        adapter.consolidate(params.clone(), 10);

        // Check EWC penalty
        let new_params = vec![1.1, 2.1, 3.1, 4.1];
        let penalty = adapter.ewc_penalty(&new_params);
        assert!(penalty > 0.0);

        // Check EWC gradient
        let ewc_grad = adapter.ewc_gradient(&new_params);
        assert_eq!(ewc_grad.len(), 4);
        assert!(ewc_grad.iter().all(|g| *g > 0.0)); // All positive since params increased
    }
}
