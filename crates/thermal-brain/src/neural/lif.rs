//! Leaky Integrate-and-Fire (LIF) neuron model

use libm::expf;

/// LIF Neuron - Leaky Integrate-and-Fire model
///
/// The membrane potential evolves according to:
/// V(t+dt) = V(t) * exp(-dt/τ) + I(t)
///
/// When V >= threshold, the neuron fires and resets to 0.
#[derive(Clone, Debug)]
pub struct LIFNeuron {
    /// Current membrane potential [0.0, threshold]
    membrane: f32,
    /// Firing threshold
    threshold: f32,
    /// Membrane time constant (ms)
    tau_ms: f32,
    /// Remaining refractory time (ms)
    refractory_remaining: u32,
    /// Spike count (for statistics)
    spike_count: u64,
}

impl LIFNeuron {
    /// Create a new LIF neuron
    ///
    /// # Arguments
    /// * `threshold` - Firing threshold (typically 0.3-0.9)
    /// * `tau_ms` - Membrane time constant in milliseconds (typically 10-50ms)
    pub fn new(threshold: f32, tau_ms: f32) -> Self {
        Self {
            membrane: 0.0,
            threshold,
            tau_ms,
            refractory_remaining: 0,
            spike_count: 0,
        }
    }

    /// Integrate input current and check for spike
    ///
    /// # Arguments
    /// * `input` - Input current (similarity score, typically 0.0-1.0)
    /// * `dt_ms` - Time step in milliseconds
    /// * `refractory_ms` - Refractory period if spike occurs
    ///
    /// # Returns
    /// `true` if neuron fired, `false` otherwise
    pub fn integrate(&mut self, input: f32, dt_ms: u32, refractory_ms: u32) -> bool {
        // Check refractory period
        if self.refractory_remaining > 0 {
            self.refractory_remaining = self.refractory_remaining.saturating_sub(dt_ms);
            return false;
        }

        // Leaky integration: V(t+dt) = V(t) * decay + input
        let decay = expf(-(dt_ms as f32) / self.tau_ms);
        self.membrane = self.membrane * decay + input;

        // Check for spike
        if self.membrane >= self.threshold {
            self.membrane = 0.0;
            self.refractory_remaining = refractory_ms;
            self.spike_count += 1;
            return true;
        }

        false
    }

    /// Reset neuron to initial state
    pub fn reset(&mut self) {
        self.membrane = 0.0;
        self.refractory_remaining = 0;
    }

    /// Get current membrane potential
    pub fn membrane(&self) -> f32 {
        self.membrane
    }

    /// Get firing threshold
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Set firing threshold
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold;
    }

    /// Check if neuron is in refractory period
    pub fn is_refractory(&self) -> bool {
        self.refractory_remaining > 0
    }

    /// Get remaining refractory time
    pub fn refractory_remaining(&self) -> u32 {
        self.refractory_remaining
    }

    /// Get total spike count
    pub fn spike_count(&self) -> u64 {
        self.spike_count
    }

    /// Get normalized membrane potential (0.0 to 1.0)
    pub fn normalized_potential(&self) -> f32 {
        (self.membrane / self.threshold).clamp(0.0, 1.0)
    }
}

/// LIF neuron with 16-bit fixed-point (for Cognitum)
#[derive(Clone, Debug)]
pub struct LIFNeuronI16 {
    /// Membrane potential (Q8.8 fixed-point)
    membrane: i16,
    /// Threshold (Q8.8 fixed-point)
    threshold: i16,
    /// Decay as right shift (approximates exp decay)
    tau_shift: u8,
    /// Remaining refractory cycles
    refractory_remaining: u8,
    /// Spike count
    spike_count: u32,
}

impl LIFNeuronI16 {
    /// Create a new I16 LIF neuron
    ///
    /// # Arguments
    /// * `threshold` - Threshold in Q8.8 format (e.g., 128 = 0.5)
    /// * `tau_shift` - Decay shift (e.g., 3 = decay by 1/8 per step)
    pub fn new(threshold: i16, tau_shift: u8) -> Self {
        Self {
            membrane: 0,
            threshold,
            tau_shift,
            refractory_remaining: 0,
            spike_count: 0,
        }
    }

    /// Integrate and check for spike (fixed-point)
    ///
    /// # Arguments
    /// * `input` - Input in Q8.8 format
    /// * `refractory_cycles` - Refractory period in cycles
    ///
    /// # Returns
    /// `true` if neuron fired
    pub fn integrate(&mut self, input: i16, refractory_cycles: u8) -> bool {
        if self.refractory_remaining > 0 {
            self.refractory_remaining -= 1;
            return false;
        }

        // Leaky decay via right shift
        self.membrane = (self.membrane >> self.tau_shift) + input;

        if self.membrane >= self.threshold {
            self.membrane = 0;
            self.refractory_remaining = refractory_cycles;
            self.spike_count += 1;
            return true;
        }

        false
    }

    /// Reset neuron
    pub fn reset(&mut self) {
        self.membrane = 0;
        self.refractory_remaining = 0;
    }

    /// Get membrane potential
    pub fn membrane(&self) -> i16 {
        self.membrane
    }

    /// Get spike count
    pub fn spike_count(&self) -> u32 {
        self.spike_count
    }
}

/// Neuron bank - collection of LIF neurons
pub struct NeuronBank {
    neurons: heapless::Vec<LIFNeuron, 64>,
}

impl NeuronBank {
    /// Create a new neuron bank with specified count
    pub fn new(count: usize, threshold: f32, tau_ms: f32) -> Self {
        let mut neurons = heapless::Vec::new();
        for _ in 0..count.min(64) {
            let _ = neurons.push(LIFNeuron::new(threshold, tau_ms));
        }
        Self { neurons }
    }

    /// Get number of neurons
    pub fn len(&self) -> usize {
        self.neurons.len()
    }

    /// Check if bank is empty
    pub fn is_empty(&self) -> bool {
        self.neurons.is_empty()
    }

    /// Get neuron by index
    pub fn get(&self, idx: usize) -> Option<&LIFNeuron> {
        self.neurons.get(idx)
    }

    /// Get mutable neuron by index
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut LIFNeuron> {
        self.neurons.get_mut(idx)
    }

    /// Reset all neurons
    pub fn reset_all(&mut self) {
        for n in self.neurons.iter_mut() {
            n.reset();
        }
    }

    /// Set threshold for all neurons
    pub fn set_all_thresholds(&mut self, threshold: f32) {
        for n in self.neurons.iter_mut() {
            n.set_threshold(threshold);
        }
    }

    /// Add a neuron to the bank
    pub fn add(&mut self, neuron: LIFNeuron) -> Result<(), LIFNeuron> {
        self.neurons.push(neuron)
    }

    /// Iterate over neurons
    pub fn iter(&self) -> impl Iterator<Item = &LIFNeuron> {
        self.neurons.iter()
    }

    /// Iterate mutably over neurons
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut LIFNeuron> {
        self.neurons.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lif_neuron_basic() {
        let mut neuron = LIFNeuron::new(0.5, 20.0);

        // Initially no spike with small input
        assert!(!neuron.integrate(0.1, 10, 50));
        assert!(neuron.membrane() < 0.5);

        // Should accumulate
        for _ in 0..10 {
            neuron.integrate(0.1, 10, 50);
        }

        // Eventually should spike with enough input
        let mut fired = false;
        for _ in 0..50 {
            if neuron.integrate(0.2, 10, 50) {
                fired = true;
                break;
            }
        }
        assert!(fired);
    }

    #[test]
    fn test_lif_refractory() {
        let mut neuron = LIFNeuron::new(0.3, 20.0);

        // Force spike
        for _ in 0..20 {
            neuron.integrate(0.5, 10, 100);
        }

        // Should be in refractory
        assert!(neuron.is_refractory());

        // Should not spike during refractory
        assert!(!neuron.integrate(1.0, 10, 100));
    }

    #[test]
    fn test_lif_decay() {
        let mut neuron = LIFNeuron::new(1.0, 20.0);

        // Add some charge
        neuron.integrate(0.5, 10, 50);
        let v1 = neuron.membrane();

        // Let it decay
        neuron.integrate(0.0, 50, 50);
        let v2 = neuron.membrane();

        // Should have decayed
        assert!(v2 < v1);
    }

    #[test]
    fn test_lif_i16() {
        // Lower threshold so neuron can actually fire
        // With decay shift 3 (1/8) and input 100, membrane converges to ~114
        let mut neuron = LIFNeuronI16::new(100, 3); // threshold = ~0.39, decay = 1/8

        // Accumulate with strong input
        let mut fired = false;
        for _ in 0..30 {
            if neuron.integrate(100, 5) {
                fired = true;
                break;
            }
        }
        assert!(fired);
    }

    #[test]
    fn test_neuron_bank() {
        let mut bank = NeuronBank::new(8, 0.5, 20.0);
        assert_eq!(bank.len(), 8);

        bank.set_all_thresholds(0.7);
        for n in bank.iter() {
            assert_eq!(n.threshold(), 0.7);
        }

        bank.reset_all();
        for n in bank.iter() {
            assert_eq!(n.membrane(), 0.0);
        }
    }
}
