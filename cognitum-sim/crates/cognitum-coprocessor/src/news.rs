//! NEWS (Neuromorphic Event-driven Weighted Spike) Coprocessor
//!
//! This module implements a spiking neural network coprocessor for the Cognitum ASIC,
//! providing event-driven neuromorphic computation with the following features:
//!
//! - **Leaky Integrate-and-Fire Neurons**: Biologically-inspired neuron model
//! - **256 Neurons per Tile**: Scalable architecture for large networks
//! - **STDP Learning**: Spike-Timing-Dependent Plasticity for adaptive weights
//! - **Event-Driven Processing**: Efficient spike-based computation
//! - **Packet-Based Communication**: Integration with RaceWay interconnect

#![warn(missing_docs)]

use std::collections::VecDeque;

/// Maximum number of neurons per NEWS tile
pub const MAX_NEURONS: usize = 256;

/// Maximum synaptic connections per neuron
pub const MAX_SYNAPSES: usize = 256;

/// Default refractory period (cycles)
pub const DEFAULT_REFRACTORY_PERIOD: u8 = 5;

/// Spike event representing a neuron firing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpikeEvent {
    /// Source neuron ID (0-255)
    pub source: u8,
    /// Target neuron ID (0-255)
    pub target: u8,
    /// Timestamp when spike should be delivered
    pub time: u64,
    /// Synaptic weight for this connection
    pub weight: i16,
}

/// Leaky Integrate-and-Fire neuron model
///
/// This implements a simplified but biologically-plausible neuron model with:
/// - Membrane potential that integrates incoming spikes
/// - Exponential leak over time
/// - Threshold-based spike generation
/// - Refractory period after firing
#[derive(Debug, Clone)]
pub struct LeakyIntegrateFireNeuron {
    /// Membrane potential (mV * 256 for fixed-point)
    potential: i32,

    /// Spike threshold (mV * 256)
    threshold: i32,

    /// Leak rate (0-255, where 255 = no leak)
    leak_rate: u8,

    /// Refractory period counter (cycles remaining)
    refractory: u8,

    /// Refractory period duration (cycles)
    refractory_period: u8,

    /// Synaptic weights indexed by source neuron ID
    weights: Vec<i16>,

    /// Resting potential (mV * 256)
    resting_potential: i32,

    /// Learning rate for STDP (0-255)
    learning_rate: u8,

    /// Spike trace for STDP (exponentially decaying)
    spike_trace: i32,

    /// Last spike time for STDP
    last_spike_time: Option<u64>,
}

impl LeakyIntegrateFireNeuron {
    /// Create a new neuron with default parameters
    ///
    /// # Parameters
    /// - `threshold`: Spike threshold in mV * 256 (typically 20 mV = 5120)
    /// - `leak_rate`: Leak coefficient (0-255, where 255 = no leak, 240 = ~6% leak/step)
    pub fn new(threshold: i32, leak_rate: u8) -> Self {
        Self {
            potential: 0,
            threshold,
            leak_rate,
            refractory: 0,
            refractory_period: DEFAULT_REFRACTORY_PERIOD,
            weights: vec![0; MAX_SYNAPSES],
            resting_potential: 0,
            learning_rate: 16, // ~6% learning rate
            spike_trace: 0,
            last_spike_time: None,
        }
    }

    /// Create neuron with custom parameters
    pub fn with_params(
        threshold: i32,
        leak_rate: u8,
        resting_potential: i32,
        learning_rate: u8,
    ) -> Self {
        Self {
            potential: resting_potential,
            threshold,
            leak_rate,
            refractory: 0,
            refractory_period: DEFAULT_REFRACTORY_PERIOD,
            weights: vec![0; MAX_SYNAPSES],
            resting_potential,
            learning_rate,
            spike_trace: 0,
            last_spike_time: None,
        }
    }

    /// Set synaptic weight for a specific source neuron
    pub fn set_weight(&mut self, source_id: u8, weight: i16) {
        if (source_id as usize) < MAX_SYNAPSES {
            self.weights[source_id as usize] = weight;
        }
    }

    /// Get synaptic weight for a specific source neuron
    pub fn get_weight(&self, source_id: u8) -> i16 {
        if (source_id as usize) < MAX_SYNAPSES {
            self.weights[source_id as usize]
        } else {
            0
        }
    }

    /// Get current membrane potential
    pub fn potential(&self) -> i32 {
        self.potential
    }

    /// Get spike threshold
    pub fn threshold(&self) -> i32 {
        self.threshold
    }

    /// Check if neuron is in refractory period
    pub fn is_refractory(&self) -> bool {
        self.refractory > 0
    }

    /// Update neuron state for one time step
    ///
    /// Returns true if neuron fired a spike
    pub fn update(&mut self, current_time: u64) -> bool {
        // Handle refractory period
        if self.refractory > 0 {
            self.refractory -= 1;
            return false;
        }

        // Check for spike threshold BEFORE leak (spike decision at peak potential)
        if self.potential >= self.threshold {
            self.fire_spike(current_time);
            return true;
        }

        // Apply leak toward resting potential
        // leak_rate: 255 = minimal leak, 0 = maximum leak
        let diff = self.potential - self.resting_potential;
        let retained = (diff * self.leak_rate as i32) / 256;
        self.potential = self.resting_potential + retained;

        // Decay spike trace for STDP
        self.spike_trace = (self.spike_trace * 240) / 256; // ~6% decay

        false
    }

    /// Receive spike from another neuron
    ///
    /// # Parameters
    /// - `source_id`: ID of neuron that fired (0-255)
    /// - `current_time`: Current simulation time
    /// - `weight_override`: Optional weight override (for external inputs)
    pub fn receive_spike(&mut self, source_id: u8, current_time: u64, weight_override: Option<i16>) {
        if self.refractory > 0 {
            return;
        }

        let weight = if let Some(w) = weight_override {
            w
        } else if (source_id as usize) < MAX_SYNAPSES {
            self.weights[source_id as usize]
        } else {
            0
        };

        // Apply synaptic input
        self.potential = self.potential.saturating_add(weight as i32);

        // STDP only for internal connections (not external inputs)
        if weight_override.is_none() && (source_id as usize) < MAX_SYNAPSES {
            // STDP: Potentiate if we fired recently (pre-before-post)
            if let Some(last_spike) = self.last_spike_time {
                if current_time > last_spike {
                    let dt = (current_time - last_spike) as i32;
                    // Potentiation: weight increase for causal spikes
                    if dt < 20 {
                        let dw = ((self.learning_rate as i32) * self.spike_trace) / 256;
                        let new_weight = (weight as i32 + dw).clamp(-32768, 32767);
                        self.weights[source_id as usize] = new_weight as i16;
                    }
                }
            }

            // STDP: Depress if pre-spike arrives (post-before-pre)
            if self.spike_trace > 0 {
                let dw = -((self.learning_rate as i32) * self.spike_trace) / 512;
                let new_weight = (weight as i32 + dw).clamp(-32768, 32767);
                self.weights[source_id as usize] = new_weight as i16;
            }
        }
    }

    /// Fire a spike
    fn fire_spike(&mut self, current_time: u64) {
        self.potential = self.resting_potential;
        self.refractory = self.refractory_period;
        self.spike_trace = 32767; // Max trace value
        self.last_spike_time = Some(current_time);
    }

    /// Reset neuron to initial state
    pub fn reset(&mut self) {
        self.potential = self.resting_potential;
        self.refractory = 0;
        self.spike_trace = 0;
        self.last_spike_time = None;
    }
}

/// NEWS Coprocessor managing a tile of 256 neurons
///
/// This implements the complete neuromorphic coprocessor with:
/// - 256 leaky integrate-and-fire neurons
/// - Event-driven spike processing
/// - STDP learning
/// - Packet-based spike routing
#[derive(Debug)]
pub struct NewsCoprocessor {
    /// Array of 256 neurons
    neurons: Vec<LeakyIntegrateFireNeuron>,

    /// Outgoing connections: for each neuron, list of (target, weight_index) pairs
    /// This allows efficient spike routing when a neuron fires
    outgoing_connections: Vec<Vec<u8>>,

    /// Spike event queue (sorted by delivery time)
    spike_queue: VecDeque<SpikeEvent>,

    /// Current simulation time (cycles)
    time: u64,

    /// Spike output buffer (for RaceWay packets)
    output_spikes: Vec<SpikeEvent>,

    /// Statistics
    total_spikes: u64,
    total_updates: u64,
}

impl NewsCoprocessor {
    /// Create a new NEWS coprocessor with default neurons
    pub fn new() -> Self {
        let neurons = (0..MAX_NEURONS)
            .map(|_| LeakyIntegrateFireNeuron::new(5120, 240)) // 20mV threshold, ~6% leak
            .collect();

        Self {
            neurons,
            outgoing_connections: vec![Vec::new(); MAX_NEURONS],
            spike_queue: VecDeque::new(),
            time: 0,
            output_spikes: Vec::new(),
            total_spikes: 0,
            total_updates: 0,
        }
    }

    /// Get number of neurons in this tile
    pub fn neuron_count(&self) -> usize {
        self.neurons.len()
    }

    /// Get reference to a specific neuron
    pub fn neuron(&self, id: u8) -> Option<&LeakyIntegrateFireNeuron> {
        self.neurons.get(id as usize)
    }

    /// Get mutable reference to a specific neuron
    pub fn neuron_mut(&mut self, id: u8) -> Option<&mut LeakyIntegrateFireNeuron> {
        self.neurons.get_mut(id as usize)
    }

    /// Set synaptic connection between neurons
    ///
    /// # Parameters
    /// - `source`: Source neuron ID (0-255)
    /// - `target`: Target neuron ID (0-255)
    /// - `weight`: Synaptic weight (-32768 to 32767)
    pub fn connect(&mut self, source: u8, target: u8, weight: i16) {
        if let Some(neuron) = self.neuron_mut(target) {
            neuron.set_weight(source, weight);

            // Update outgoing connections for efficient spike routing
            if (source as usize) < MAX_NEURONS {
                let outgoing = &mut self.outgoing_connections[source as usize];
                if !outgoing.contains(&target) {
                    outgoing.push(target);
                }
            }
        }
    }

    /// Inject external spike into the network
    ///
    /// # Parameters
    /// - `target`: Target neuron ID
    /// - `weight`: Input weight
    pub fn inject_spike(&mut self, target: u8, weight: i16) {
        let event = SpikeEvent {
            source: 255, // External input marker
            target,
            time: self.time,
            weight,
        };
        self.spike_queue.push_back(event);
    }

    /// Step simulation forward by one time step
    ///
    /// Returns vector of output spikes generated this cycle
    pub fn step(&mut self) -> Vec<SpikeEvent> {
        self.output_spikes.clear();
        self.total_updates += 1;

        // Process pending spike events for current time FIRST
        // This delivers external inputs and inter-neuron spikes before neurons update
        let current_time = self.time;
        while let Some(event) = self.spike_queue.front() {
            if event.time <= current_time {
                let event = self.spike_queue.pop_front().unwrap();
                if let Some(neuron) = self.neuron_mut(event.target) {
                    // Use weight from event if source is external (255) or weight is non-zero
                    let weight_override = if event.source == 255 || event.weight != 0 {
                        Some(event.weight)
                    } else {
                        None
                    };
                    neuron.receive_spike(event.source, current_time, weight_override);
                }
            } else {
                break;
            }
        }

        // Process all neurons for this time step
        for neuron_id in 0..self.neurons.len() {
            if self.neurons[neuron_id].update(self.time) {
                // Neuron fired - generate output spikes to all connected neurons
                self.total_spikes += 1;

                // Create spike events for all outgoing connections
                for &target_id in &self.outgoing_connections[neuron_id] {
                    let weight = self.neurons[target_id as usize].get_weight(neuron_id as u8);
                    if weight != 0 {
                        let event = SpikeEvent {
                            source: neuron_id as u8,
                            target: target_id,
                            time: self.time + 1, // Deliver next cycle
                            weight,
                        };
                        self.spike_queue.push_back(event);
                    }
                }

                // Add to output buffer for external routing
                self.output_spikes.push(SpikeEvent {
                    source: neuron_id as u8,
                    target: 0,
                    time: self.time,
                    weight: 0,
                });
            }
        }

        self.time += 1;
        self.output_spikes.clone()
    }

    /// Run simulation for multiple steps
    pub fn run(&mut self, steps: u64) -> u64 {
        let mut spike_count = 0;
        for _ in 0..steps {
            spike_count += self.step().len() as u64;
        }
        spike_count
    }

    /// Get current simulation time
    pub fn time(&self) -> u64 {
        self.time
    }

    /// Get total spikes generated
    pub fn total_spikes(&self) -> u64 {
        self.total_spikes
    }

    /// Get pending spike queue length
    pub fn queue_length(&self) -> usize {
        self.spike_queue.len()
    }

    /// Reset entire coprocessor
    pub fn reset(&mut self) {
        for neuron in &mut self.neurons {
            neuron.reset();
        }
        self.spike_queue.clear();
        self.output_spikes.clear();
        self.time = 0;
        self.total_spikes = 0;
        self.total_updates = 0;
        // Note: we don't reset connections, only dynamic state
    }

    /// Get average firing rate (spikes per neuron per step)
    pub fn average_firing_rate(&self) -> f64 {
        if self.total_updates == 0 {
            0.0
        } else {
            self.total_spikes as f64 / (self.total_updates as f64 * MAX_NEURONS as f64)
        }
    }
}

impl Default for NewsCoprocessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neuron_creation() {
        let neuron = LeakyIntegrateFireNeuron::new(5120, 240);
        assert_eq!(neuron.potential(), 0);
        assert_eq!(neuron.threshold(), 5120);
        assert!(!neuron.is_refractory());
    }

    #[test]
    fn test_neuron_spike() {
        let mut neuron = LeakyIntegrateFireNeuron::new(1000, 255);

        // Should not spike initially
        assert!(!neuron.update(0));

        // Inject enough current to reach threshold
        neuron.set_weight(0, 1000);
        neuron.receive_spike(0, 0, None);

        // Should spike
        assert!(neuron.update(1));
        assert_eq!(neuron.potential(), 0);
        assert!(neuron.is_refractory());
    }

    #[test]
    fn test_neuron_leak() {
        let mut neuron = LeakyIntegrateFireNeuron::new(10000, 240);
        neuron.set_weight(0, 1000);
        neuron.receive_spike(0, 0, None);

        let initial_potential = neuron.potential();
        assert!(initial_potential > 0);

        // Leak should reduce potential over time
        neuron.update(0);
        let after_leak = neuron.potential();
        assert!(after_leak < initial_potential);
    }

    #[test]
    fn test_coprocessor_creation() {
        let news = NewsCoprocessor::new();
        assert_eq!(news.neuron_count(), MAX_NEURONS);
        assert_eq!(news.time(), 0);
        assert_eq!(news.total_spikes(), 0);
    }

    #[test]
    fn test_connection() {
        let mut news = NewsCoprocessor::new();
        news.connect(0, 1, 1000);

        assert_eq!(news.neuron(1).unwrap().get_weight(0), 1000);
    }

    #[test]
    fn test_spike_propagation() {
        let mut news = NewsCoprocessor::new();

        // Create simple connection: neuron 0 -> neuron 1
        news.connect(0, 1, 2000);

        // Inject spike to neuron 0
        news.inject_spike(0, 5120);

        // Step 1: Neuron 0 should spike
        let spikes = news.step();
        assert!(!spikes.is_empty());

        // Step 2: Neuron 1 should receive spike
        news.step();

        assert!(news.total_spikes() > 0);
    }
}
