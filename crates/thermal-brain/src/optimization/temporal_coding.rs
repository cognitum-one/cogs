//! Temporal Coding
//!
//! Use spike timing, not just rate, to encode information.
//! Achieves higher information density and faster processing.
//!
//! Coding schemes:
//! - Time-to-first-spike: Information in spike latency
//! - Phase coding: Information in spike phase relative to oscillation
//! - Rank order: Information in relative spike order
//! - Inter-spike interval: Information in time between spikes

use heapless::Vec as HVec;

/// Maximum spikes per neuron
const MAX_SPIKES: usize = 64;

/// Maximum neurons
const MAX_NEURONS: usize = 32;

/// Temporal coding scheme
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodingScheme {
    /// Rate coding (traditional)
    Rate,
    /// Time-to-first-spike
    TimeToFirstSpike,
    /// Phase coding relative to oscillation
    Phase,
    /// Rank order coding
    RankOrder,
    /// Inter-spike interval
    InterSpikeInterval,
}

/// Spike timing information
#[derive(Clone, Copy, Debug)]
pub struct SpikeTiming {
    /// Neuron ID
    pub neuron_id: u8,
    /// Spike time (microseconds from start)
    pub time_us: u32,
    /// Phase (0.0 to 1.0, for phase coding)
    pub phase: f32,
}

impl SpikeTiming {
    /// Create new spike timing
    pub fn new(neuron_id: u8, time_us: u32) -> Self {
        Self {
            neuron_id,
            time_us,
            phase: 0.0,
        }
    }

    /// Create with phase
    pub fn with_phase(neuron_id: u8, time_us: u32, phase: f32) -> Self {
        Self {
            neuron_id,
            time_us,
            phase,
        }
    }
}

/// Temporal code (encoded representation)
#[derive(Clone, Debug)]
pub struct TemporalCode {
    /// Coding scheme used
    pub scheme: CodingScheme,
    /// Spike timings
    pub spikes: HVec<SpikeTiming, MAX_SPIKES>,
    /// Encoding window (microseconds)
    pub window_us: u32,
    /// Reference phase (for phase coding)
    pub reference_phase: f32,
}

impl TemporalCode {
    /// Create empty temporal code
    pub fn new(scheme: CodingScheme, window_us: u32) -> Self {
        Self {
            scheme,
            spikes: HVec::new(),
            window_us,
            reference_phase: 0.0,
        }
    }

    /// Add a spike
    pub fn add_spike(&mut self, timing: SpikeTiming) -> bool {
        if self.spikes.is_full() {
            return false;
        }
        let _ = self.spikes.push(timing);
        true
    }

    /// Get spike count
    pub fn spike_count(&self) -> usize {
        self.spikes.len()
    }

    /// Get first spike time for a neuron
    pub fn first_spike_time(&self, neuron_id: u8) -> Option<u32> {
        self.spikes.iter()
            .filter(|s| s.neuron_id == neuron_id)
            .map(|s| s.time_us)
            .min()
    }

    /// Get spike times for a neuron
    pub fn neuron_spikes(&self, neuron_id: u8) -> impl Iterator<Item = &SpikeTiming> {
        self.spikes.iter().filter(move |s| s.neuron_id == neuron_id)
    }
}

impl Default for TemporalCode {
    fn default() -> Self {
        Self::new(CodingScheme::Rate, 1000)
    }
}

/// Temporal encoder configuration
#[derive(Clone, Copy, Debug)]
pub struct TemporalConfig {
    /// Coding scheme
    pub scheme: CodingScheme,
    /// Encoding window (microseconds)
    pub window_us: u32,
    /// Oscillation period for phase coding (microseconds)
    pub oscillation_period_us: u32,
    /// Maximum latency for time-to-first-spike (microseconds)
    pub max_latency_us: u32,
    /// Minimum inter-spike interval (microseconds)
    pub min_isi_us: u32,
}

impl Default for TemporalConfig {
    fn default() -> Self {
        Self {
            scheme: CodingScheme::TimeToFirstSpike,
            window_us: 10000,           // 10ms
            oscillation_period_us: 1000, // 1ms (1kHz gamma)
            max_latency_us: 5000,        // 5ms
            min_isi_us: 100,             // 100us refractory
        }
    }
}

/// Temporal encoder/decoder
pub struct TemporalEncoder {
    config: TemporalConfig,
    /// Current oscillation phase
    phase: f32,
    /// Phase velocity (radians per microsecond)
    phase_velocity: f32,
}

impl TemporalEncoder {
    /// Create a new temporal encoder
    pub fn new(config: TemporalConfig) -> Self {
        let phase_velocity = 2.0 * core::f32::consts::PI / config.oscillation_period_us as f32;
        Self {
            config,
            phase: 0.0,
            phase_velocity,
        }
    }

    /// Encode values using time-to-first-spike
    ///
    /// Higher values = shorter latency
    pub fn encode_ttfs(&self, values: &[f32]) -> TemporalCode {
        let mut code = TemporalCode::new(CodingScheme::TimeToFirstSpike, self.config.window_us);

        for (neuron_id, &value) in values.iter().enumerate().take(MAX_NEURONS) {
            if value > 0.0 {
                // Latency inversely proportional to value
                let normalized = value.clamp(0.0, 1.0);
                let latency = ((1.0 - normalized) * self.config.max_latency_us as f32) as u32;

                let timing = SpikeTiming::new(neuron_id as u8, latency);
                code.add_spike(timing);
            }
        }

        code
    }

    /// Encode values using phase coding
    ///
    /// Value determines spike phase within oscillation
    pub fn encode_phase(&self, values: &[f32], time_us: u32) -> TemporalCode {
        let mut code = TemporalCode::new(CodingScheme::Phase, self.config.window_us);
        code.reference_phase = (time_us as f32 * self.phase_velocity) % (2.0 * core::f32::consts::PI);

        for (neuron_id, &value) in values.iter().enumerate().take(MAX_NEURONS) {
            if value > 0.0 {
                // Phase proportional to value
                let phase = value.clamp(0.0, 1.0);
                let spike_phase = phase * 2.0 * core::f32::consts::PI;

                // Convert phase to time
                let phase_offset = spike_phase / self.phase_velocity;
                let spike_time = time_us + phase_offset as u32;

                let timing = SpikeTiming::with_phase(neuron_id as u8, spike_time, phase);
                code.add_spike(timing);
            }
        }

        code
    }

    /// Encode values using rank order coding
    ///
    /// Information is in the relative order of spikes
    pub fn encode_rank_order(&self, values: &[f32]) -> TemporalCode {
        let mut code = TemporalCode::new(CodingScheme::RankOrder, self.config.window_us);

        // Create (index, value) pairs and sort by value (descending)
        let mut indexed: HVec<(usize, f32), MAX_NEURONS> = values.iter()
            .enumerate()
            .take(MAX_NEURONS)
            .filter(|(_, &v)| v > 0.0)
            .map(|(i, &v)| (i, v))
            .collect();

        // Sort by value descending (bubble sort for heapless)
        for i in 0..indexed.len() {
            for j in i + 1..indexed.len() {
                if indexed[i].1 < indexed[j].1 {
                    indexed.swap(i, j);
                }
            }
        }

        // Assign spike times based on rank
        let time_step = self.config.max_latency_us / (indexed.len().max(1) as u32);

        for (rank, &(neuron_id, _)) in indexed.iter().enumerate() {
            let time = rank as u32 * time_step;
            let timing = SpikeTiming::new(neuron_id as u8, time);
            code.add_spike(timing);
        }

        code
    }

    /// Encode values using inter-spike intervals
    ///
    /// Value determines interval between spikes
    pub fn encode_isi(&self, value: f32, neuron_id: u8) -> TemporalCode {
        let mut code = TemporalCode::new(CodingScheme::InterSpikeInterval, self.config.window_us);

        if value <= 0.0 {
            return code;
        }

        // Higher value = shorter ISI = more spikes
        let normalized = value.clamp(0.0, 1.0);
        let isi = self.config.min_isi_us + ((1.0 - normalized) * 1000.0) as u32;

        let mut time = 0u32;
        while time < self.config.window_us {
            let timing = SpikeTiming::new(neuron_id, time);
            if !code.add_spike(timing) {
                break;
            }
            time += isi;
        }

        code
    }

    /// Decode time-to-first-spike to values
    pub fn decode_ttfs(&self, code: &TemporalCode, num_neurons: usize) -> HVec<f32, MAX_NEURONS> {
        let mut values = HVec::new();

        for neuron_id in 0..num_neurons.min(MAX_NEURONS) {
            let value = if let Some(first_time) = code.first_spike_time(neuron_id as u8) {
                // Latency to value
                let normalized_latency = first_time as f32 / self.config.max_latency_us as f32;
                1.0 - normalized_latency.clamp(0.0, 1.0)
            } else {
                0.0
            };
            let _ = values.push(value);
        }

        values
    }

    /// Decode rank order to values
    pub fn decode_rank_order(&self, code: &TemporalCode, num_neurons: usize) -> HVec<f32, MAX_NEURONS> {
        let mut values: HVec<f32, MAX_NEURONS> = (0..num_neurons.min(MAX_NEURONS))
            .map(|_| 0.0)
            .collect();

        // Sort spikes by time
        let mut sorted_spikes = code.spikes.clone();
        for i in 0..sorted_spikes.len() {
            for j in i + 1..sorted_spikes.len() {
                if sorted_spikes[i].time_us > sorted_spikes[j].time_us {
                    sorted_spikes.swap(i, j);
                }
            }
        }

        // Assign values based on rank (earlier = higher value)
        let num_spikes = sorted_spikes.len();
        for (rank, spike) in sorted_spikes.iter().enumerate() {
            if (spike.neuron_id as usize) < values.len() {
                // Earlier spikes get higher values
                let value = 1.0 - (rank as f32 / num_spikes.max(1) as f32);
                values[spike.neuron_id as usize] = value;
            }
        }

        values
    }

    /// Decode ISI to value
    pub fn decode_isi(&self, code: &TemporalCode, neuron_id: u8) -> f32 {
        let spikes: HVec<u32, MAX_SPIKES> = code.neuron_spikes(neuron_id)
            .map(|s| s.time_us)
            .collect();

        if spikes.len() < 2 {
            return 0.0;
        }

        // Calculate average ISI
        let mut total_isi = 0u32;
        for i in 1..spikes.len() {
            total_isi += spikes[i] - spikes[i - 1];
        }
        let avg_isi = total_isi / (spikes.len() - 1) as u32;

        // ISI to value (shorter ISI = higher value)
        let max_isi = self.config.min_isi_us + 1000;
        1.0 - ((avg_isi as f32 - self.config.min_isi_us as f32) / (max_isi - self.config.min_isi_us) as f32).clamp(0.0, 1.0)
    }

    /// Update oscillation phase (for phase coding)
    pub fn advance_phase(&mut self, dt_us: u32) {
        self.phase += self.phase_velocity * dt_us as f32;
        self.phase %= 2.0 * core::f32::consts::PI;
    }

    /// Get current phase
    pub fn phase(&self) -> f32 {
        self.phase
    }

    /// Reset phase
    pub fn reset_phase(&mut self) {
        self.phase = 0.0;
    }
}

impl Default for TemporalEncoder {
    fn default() -> Self {
        Self::new(TemporalConfig::default())
    }
}

/// Temporal pattern matcher
///
/// Matches spike patterns considering timing
pub struct TemporalMatcher {
    /// Stored patterns
    patterns: HVec<TemporalCode, 16>,
    /// Timing tolerance (microseconds)
    tolerance_us: u32,
}

impl TemporalMatcher {
    /// Create a new temporal matcher
    pub fn new(tolerance_us: u32) -> Self {
        Self {
            patterns: HVec::new(),
            tolerance_us,
        }
    }

    /// Store a pattern
    pub fn store_pattern(&mut self, pattern: TemporalCode) -> bool {
        if self.patterns.is_full() {
            return false;
        }
        let _ = self.patterns.push(pattern);
        true
    }

    /// Match against stored patterns
    ///
    /// Returns (pattern_index, similarity) for best match
    pub fn match_pattern(&self, input: &TemporalCode) -> Option<(usize, f32)> {
        let mut best_match = None;
        let mut best_similarity = 0.0f32;

        for (idx, pattern) in self.patterns.iter().enumerate() {
            let similarity = self.compute_similarity(input, pattern);
            if similarity > best_similarity {
                best_similarity = similarity;
                best_match = Some((idx, similarity));
            }
        }

        best_match
    }

    /// Compute similarity between two temporal codes
    fn compute_similarity(&self, a: &TemporalCode, b: &TemporalCode) -> f32 {
        if a.spikes.is_empty() || b.spikes.is_empty() {
            return 0.0;
        }

        let mut matched = 0;
        let mut total = a.spikes.len() + b.spikes.len();

        for spike_a in a.spikes.iter() {
            for spike_b in b.spikes.iter() {
                if spike_a.neuron_id == spike_b.neuron_id {
                    let time_diff = (spike_a.time_us as i32 - spike_b.time_us as i32).unsigned_abs();
                    if time_diff <= self.tolerance_us {
                        matched += 2; // Count both as matched
                    }
                }
            }
        }

        matched as f32 / total as f32
    }

    /// Get number of stored patterns
    pub fn pattern_count(&self) -> usize {
        self.patterns.len()
    }

    /// Clear all patterns
    pub fn clear(&mut self) {
        self.patterns.clear();
    }
}

impl Default for TemporalMatcher {
    fn default() -> Self {
        Self::new(500) // 500us tolerance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ttfs_encoding() {
        let config = TemporalConfig::default();
        let encoder = TemporalEncoder::new(config);

        let values = [0.9, 0.5, 0.1, 0.0];
        let code = encoder.encode_ttfs(&values);

        assert_eq!(code.spike_count(), 3); // 0.0 doesn't spike

        // Higher value = earlier spike
        let t0 = code.first_spike_time(0).unwrap();
        let t1 = code.first_spike_time(1).unwrap();
        let t2 = code.first_spike_time(2).unwrap();

        assert!(t0 < t1);
        assert!(t1 < t2);
    }

    #[test]
    fn test_ttfs_decode() {
        let config = TemporalConfig::default();
        let encoder = TemporalEncoder::new(config);

        let original = [0.8, 0.4, 0.1];
        let code = encoder.encode_ttfs(&original);
        let decoded = encoder.decode_ttfs(&code, 3);

        // Values should be approximately preserved
        for (o, d) in original.iter().zip(decoded.iter()) {
            assert!((o - d).abs() < 0.2);
        }
    }

    #[test]
    fn test_rank_order_encoding() {
        let config = TemporalConfig::default();
        let encoder = TemporalEncoder::new(config);

        let values = [0.3, 0.9, 0.1, 0.6];
        let code = encoder.encode_rank_order(&values);

        // Neuron 1 (0.9) should spike first
        let t1 = code.first_spike_time(1).unwrap();

        for spike in code.spikes.iter() {
            if spike.neuron_id != 1 {
                assert!(spike.time_us >= t1);
            }
        }
    }

    #[test]
    fn test_isi_encoding() {
        let config = TemporalConfig {
            window_us: 5000,
            min_isi_us: 100,
            ..Default::default()
        };
        let encoder = TemporalEncoder::new(config);

        // High value = many spikes (short ISI)
        let code_high = encoder.encode_isi(0.9, 0);

        // Low value = few spikes (long ISI)
        let code_low = encoder.encode_isi(0.1, 0);

        assert!(code_high.spike_count() > code_low.spike_count());
    }

    #[test]
    fn test_phase_encoding() {
        let config = TemporalConfig {
            oscillation_period_us: 1000,
            ..Default::default()
        };
        let encoder = TemporalEncoder::new(config);

        let values = [0.25, 0.5, 0.75];
        let code = encoder.encode_phase(&values, 0);

        assert_eq!(code.spike_count(), 3);

        // Check phases are different
        let phases: HVec<f32, 3> = code.spikes.iter().map(|s| s.phase).collect();
        assert!(phases[0] != phases[1]);
        assert!(phases[1] != phases[2]);
    }

    #[test]
    fn test_temporal_matcher() {
        let config = TemporalConfig::default();
        let encoder = TemporalEncoder::new(config);

        let mut matcher = TemporalMatcher::new(500);

        // Store a pattern
        let pattern_values = [0.8, 0.4, 0.2];
        let pattern = encoder.encode_ttfs(&pattern_values);
        matcher.store_pattern(pattern);

        // Try to match similar input
        let similar_values = [0.75, 0.35, 0.25];
        let similar = encoder.encode_ttfs(&similar_values);

        let (idx, similarity) = matcher.match_pattern(&similar).unwrap();
        assert_eq!(idx, 0);
        assert!(similarity > 0.5);
    }
}
