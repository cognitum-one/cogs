//! Core observation system for perception layer.
//!
//! This module provides the fundamental observation primitives for sensing
//! simulation state with realistic constraints like limited field of view,
//! range limitations, and measurement uncertainty.
//!
//! # Overview
//!
//! The observation system models how an agent perceives the simulation world:
//!
//! - **Full observations**: Complete state access (for testing/debugging)
//! - **Partial observations**: Limited FOV and range (realistic sensing)
//! - **Uncertainty quantification**: Each measurement includes confidence
//!
//! # Examples
//!
//! ```rust,no_run
//! use fxnn::perception::{Observer, PartialObserver, ObserverConfig};
//! use fxnn::types::{Atom, SimulationBox};
//!
//! let config = ObserverConfig::default();
//! let observer = PartialObserver::new([5.0, 5.0, 5.0], [1.0, 0.0, 0.0], config);
//!
//! let atoms = vec![Atom::default()];
//! let sim_box = SimulationBox::cubic(10.0);
//!
//! let observation = observer.observe(&atoms, &sim_box);
//! println!("Observed {} atoms", observation.visible_count());
//! ```

use crate::types::{Atom, SimulationBox};
use serde::{Deserialize, Serialize};

/// Configuration for observation parameters.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ObserverConfig {
    /// Field of view angle in radians (cone half-angle).
    /// For omnidirectional sensing, use PI (180 degrees).
    pub fov_angle: f32,

    /// Maximum sensing range in simulation units.
    pub max_range: f32,

    /// Observation update rate in Hz (observations per second).
    pub update_rate: f32,

    /// Minimum range (blind spot around observer).
    pub min_range: f32,

    /// Whether to include velocity information.
    pub observe_velocity: bool,

    /// Whether to include force information.
    pub observe_force: bool,

    /// Base uncertainty for position measurements.
    pub position_uncertainty: f32,

    /// Base uncertainty for velocity measurements.
    pub velocity_uncertainty: f32,
}

impl Default for ObserverConfig {
    fn default() -> Self {
        Self {
            fov_angle: std::f32::consts::PI, // Full sphere
            max_range: 10.0,
            update_rate: 60.0,
            min_range: 0.0,
            observe_velocity: true,
            observe_force: false,
            position_uncertainty: 0.01,
            velocity_uncertainty: 0.1,
        }
    }
}

impl ObserverConfig {
    /// Create config for omnidirectional sensing.
    pub fn omnidirectional(max_range: f32) -> Self {
        Self {
            max_range,
            fov_angle: std::f32::consts::PI,
            ..Default::default()
        }
    }

    /// Create config for directional sensing (like a camera).
    pub fn directional(fov_degrees: f32, max_range: f32) -> Self {
        Self {
            fov_angle: fov_degrees.to_radians() / 2.0,
            max_range,
            ..Default::default()
        }
    }
}

/// Individual sensor reading for a single observed entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    /// Index of the observed atom in the original array.
    pub atom_id: u32,

    /// Observed position with uncertainty.
    pub position: [f32; 3],

    /// Position uncertainty (1-sigma in each dimension).
    pub position_uncertainty: [f32; 3],

    /// Observed velocity (if available).
    pub velocity: Option<[f32; 3]>,

    /// Velocity uncertainty (if velocity observed).
    pub velocity_uncertainty: Option<[f32; 3]>,

    /// Distance from observer.
    pub distance: f32,

    /// Angle from observer's look direction (radians).
    pub angle: f32,

    /// Confidence score [0, 1] based on distance and angle.
    pub confidence: f32,

    /// Atom type identifier.
    pub atom_type: u16,
}

impl SensorReading {
    /// Calculate the total position uncertainty magnitude.
    pub fn position_uncertainty_magnitude(&self) -> f32 {
        (self.position_uncertainty[0].powi(2)
            + self.position_uncertainty[1].powi(2)
            + self.position_uncertainty[2].powi(2))
        .sqrt()
    }

    /// Check if this reading is reliable (confidence above threshold).
    pub fn is_reliable(&self, threshold: f32) -> bool {
        self.confidence >= threshold
    }
}

/// Aggregated observation data from a sensing operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationData {
    /// Individual sensor readings.
    pub readings: Vec<SensorReading>,

    /// Observer position at time of observation.
    pub observer_position: [f32; 3],

    /// Observer look direction (unit vector).
    pub observer_direction: [f32; 3],

    /// Configuration used for this observation.
    pub config: ObserverConfig,
}

impl ObservationData {
    /// Get the number of visible entities.
    pub fn visible_count(&self) -> usize {
        self.readings.len()
    }

    /// Check if any entities are visible.
    pub fn is_empty(&self) -> bool {
        self.readings.is_empty()
    }

    /// Get readings sorted by distance (nearest first).
    pub fn sorted_by_distance(&self) -> Vec<&SensorReading> {
        let mut sorted: Vec<_> = self.readings.iter().collect();
        sorted.sort_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
        sorted
    }

    /// Get readings sorted by confidence (highest first).
    pub fn sorted_by_confidence(&self) -> Vec<&SensorReading> {
        let mut sorted: Vec<_> = self.readings.iter().collect();
        sorted.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        sorted
    }

    /// Filter readings by minimum confidence.
    pub fn filter_by_confidence(&self, min_confidence: f32) -> Vec<&SensorReading> {
        self.readings
            .iter()
            .filter(|r| r.confidence >= min_confidence)
            .collect()
    }

    /// Calculate average confidence of all readings.
    pub fn average_confidence(&self) -> f32 {
        if self.readings.is_empty() {
            return 0.0;
        }
        self.readings.iter().map(|r| r.confidence).sum::<f32>() / self.readings.len() as f32
    }

    /// Get the closest reading.
    pub fn nearest(&self) -> Option<&SensorReading> {
        self.readings
            .iter()
            .min_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap())
    }

    /// Get the farthest reading.
    pub fn farthest(&self) -> Option<&SensorReading> {
        self.readings
            .iter()
            .max_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap())
    }
}

/// Observation with timestamp and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Observation {
    /// The observation data.
    pub data: ObservationData,

    /// Timestamp in simulation time units.
    pub timestamp: f64,

    /// Sequence number for ordering observations.
    pub sequence: u64,

    /// Total entropy of this observation (Shannon bits).
    pub entropy: f32,

    /// Information content (bytes) of this observation.
    pub info_bytes: usize,
}

impl Observation {
    /// Create a new observation with the given data.
    pub fn new(data: ObservationData, timestamp: f64, sequence: u64) -> Self {
        let info_bytes = Self::calculate_info_bytes(&data);
        let entropy = Self::calculate_entropy(&data);

        Self {
            data,
            timestamp,
            sequence,
            entropy,
            info_bytes,
        }
    }

    /// Calculate approximate information content in bytes.
    fn calculate_info_bytes(data: &ObservationData) -> usize {
        let per_reading = 3 * 4  // position
            + 3 * 4  // position uncertainty
            + 4      // distance
            + 4      // angle
            + 4      // confidence
            + 2;     // atom_type

        let velocity_size = if data.config.observe_velocity {
            6 * 4 // velocity + uncertainty
        } else {
            0
        };

        data.readings.len() * (per_reading + velocity_size) + 6 * 4 // observer pos + dir
    }

    /// Calculate Shannon entropy of the observation.
    fn calculate_entropy(data: &ObservationData) -> f32 {
        if data.readings.is_empty() {
            return 0.0;
        }

        // Quantize distances into bins for entropy calculation
        let distances: Vec<f32> = data.readings.iter().map(|r| r.distance).collect();
        let max_dist = data.config.max_range;

        // Use 16 bins for quantization
        let num_bins = 16;
        let mut bins = vec![0usize; num_bins];

        for &d in &distances {
            let bin = ((d / max_dist).min(0.9999) * num_bins as f32) as usize;
            bins[bin] += 1;
        }

        let n = distances.len() as f32;
        bins.iter()
            .filter(|&&count| count > 0)
            .map(|&count| {
                let p = count as f32 / n;
                -p * p.log2()
            })
            .sum()
    }

    /// Get the number of visible entities.
    pub fn visible_count(&self) -> usize {
        self.data.visible_count()
    }

    /// Check if this observation exceeds an entropy budget.
    pub fn exceeds_entropy(&self, budget: f32) -> bool {
        self.entropy > budget
    }

    /// Check if this observation exceeds a byte budget.
    pub fn exceeds_bytes(&self, budget: usize) -> bool {
        self.info_bytes > budget
    }
}

/// Partial observation with limited visibility.
///
/// This represents what an agent can actually perceive, as opposed to
/// omniscient access to the full simulation state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialObservation {
    /// The base observation.
    pub observation: Observation,

    /// Fraction of total atoms that are visible [0, 1].
    pub visibility_fraction: f32,

    /// IDs of atoms that were occluded (if occlusion checking enabled).
    pub occluded_ids: Vec<u32>,

    /// IDs of atoms that were out of range.
    pub out_of_range_ids: Vec<u32>,

    /// IDs of atoms that were outside FOV.
    pub outside_fov_ids: Vec<u32>,
}

impl PartialObservation {
    /// Get the underlying observation data.
    pub fn data(&self) -> &ObservationData {
        &self.observation.data
    }

    /// Get the number of visible entities.
    pub fn visible_count(&self) -> usize {
        self.observation.visible_count()
    }

    /// Get total number of entities that could potentially be observed.
    pub fn total_count(&self) -> usize {
        self.visible_count()
            + self.occluded_ids.len()
            + self.out_of_range_ids.len()
            + self.outside_fov_ids.len()
    }

    /// Get the timestamp of this observation.
    pub fn timestamp(&self) -> f64 {
        self.observation.timestamp
    }
}

/// Trait for observation systems.
///
/// Implementations provide different observation strategies from
/// full omniscient access to realistic partial observations.
pub trait Observer {
    /// Observe the current state of atoms.
    ///
    /// # Arguments
    ///
    /// * `atoms` - The atoms to observe
    /// * `sim_box` - The simulation box for PBC calculations
    ///
    /// # Returns
    ///
    /// An observation containing visible atom data.
    fn observe(&self, atoms: &[Atom], sim_box: &SimulationBox) -> Observation;

    /// Get the observer's current position.
    fn position(&self) -> [f32; 3];

    /// Get the observer's look direction.
    fn direction(&self) -> [f32; 3];

    /// Get the observer's configuration.
    fn config(&self) -> &ObserverConfig;

    /// Check if an atom is within the observable range.
    fn is_in_range(&self, atom: &Atom, sim_box: &SimulationBox) -> bool {
        let config = self.config();
        let pos = self.position();
        let d2 = sim_box.distance_squared(&pos, &atom.position);
        let d = d2.sqrt();
        d >= config.min_range && d <= config.max_range
    }

    /// Check if an atom is within the field of view.
    fn is_in_fov(&self, atom: &Atom, sim_box: &SimulationBox) -> bool {
        let config = self.config();
        let pos = self.position();
        let dir = self.direction();

        // Calculate displacement to atom
        let disp = sim_box.displacement(&pos, &atom.position);
        let dist = (disp[0].powi(2) + disp[1].powi(2) + disp[2].powi(2)).sqrt();

        if dist < 1e-6 {
            return true; // At observer position
        }

        // Normalize displacement
        let norm_disp = [disp[0] / dist, disp[1] / dist, disp[2] / dist];

        // Calculate angle between look direction and displacement
        let dot = dir[0] * norm_disp[0] + dir[1] * norm_disp[1] + dir[2] * norm_disp[2];
        let angle = dot.clamp(-1.0, 1.0).acos();

        angle <= config.fov_angle
    }
}

/// Partial observer with limited field of view and range.
#[derive(Debug, Clone)]
pub struct PartialObserver {
    /// Observer position in simulation space.
    position: [f32; 3],

    /// Look direction (unit vector).
    direction: [f32; 3],

    /// Observer configuration.
    config: ObserverConfig,

    /// Observation sequence counter.
    sequence: u64,

    /// Timestamp of last observation.
    last_observation_time: Option<f64>,
}

impl PartialObserver {
    /// Create a new partial observer.
    ///
    /// # Arguments
    ///
    /// * `position` - Observer position in simulation space
    /// * `direction` - Look direction (will be normalized)
    /// * `config` - Observer configuration
    pub fn new(position: [f32; 3], direction: [f32; 3], config: ObserverConfig) -> Self {
        // Normalize direction
        let mag =
            (direction[0].powi(2) + direction[1].powi(2) + direction[2].powi(2)).sqrt();
        let direction = if mag > 1e-6 {
            [direction[0] / mag, direction[1] / mag, direction[2] / mag]
        } else {
            [1.0, 0.0, 0.0] // Default to +X if zero direction given
        };

        Self {
            position,
            direction,
            config,
            sequence: 0,
            last_observation_time: None,
        }
    }

    /// Update observer position.
    pub fn set_position(&mut self, position: [f32; 3]) {
        self.position = position;
    }

    /// Update observer look direction.
    pub fn set_direction(&mut self, direction: [f32; 3]) {
        let mag =
            (direction[0].powi(2) + direction[1].powi(2) + direction[2].powi(2)).sqrt();
        if mag > 1e-6 {
            self.direction = [direction[0] / mag, direction[1] / mag, direction[2] / mag];
        }
    }

    /// Perform a partial observation with visibility tracking.
    pub fn observe_partial(
        &mut self,
        atoms: &[Atom],
        sim_box: &SimulationBox,
        current_time: f64,
    ) -> PartialObservation {
        let mut readings = Vec::new();
        let occluded_ids = Vec::new();
        let mut out_of_range_ids = Vec::new();
        let mut outside_fov_ids = Vec::new();

        for atom in atoms {
            // Calculate distance and angle
            let disp = sim_box.displacement(&self.position, &atom.position);
            let dist = (disp[0].powi(2) + disp[1].powi(2) + disp[2].powi(2)).sqrt();

            // Check range
            if dist < self.config.min_range || dist > self.config.max_range {
                out_of_range_ids.push(atom.id);
                continue;
            }

            // Check FOV
            let angle = if dist > 1e-6 {
                let norm_disp = [disp[0] / dist, disp[1] / dist, disp[2] / dist];
                let dot = self.direction[0] * norm_disp[0]
                    + self.direction[1] * norm_disp[1]
                    + self.direction[2] * norm_disp[2];
                dot.clamp(-1.0, 1.0).acos()
            } else {
                0.0
            };

            if angle > self.config.fov_angle {
                outside_fov_ids.push(atom.id);
                continue;
            }

            // Calculate confidence based on distance and angle
            let distance_factor = 1.0 - (dist / self.config.max_range).powi(2);
            let angle_factor = if self.config.fov_angle > 0.0 {
                1.0 - (angle / self.config.fov_angle).powi(2)
            } else {
                1.0
            };
            let confidence = (distance_factor * angle_factor).max(0.0);

            // Calculate uncertainty (increases with distance)
            let dist_uncertainty_factor = 1.0 + dist / self.config.max_range;
            let pos_uncertainty = self.config.position_uncertainty * dist_uncertainty_factor;

            let reading = SensorReading {
                atom_id: atom.id,
                position: atom.position,
                position_uncertainty: [pos_uncertainty; 3],
                velocity: if self.config.observe_velocity {
                    Some(atom.velocity)
                } else {
                    None
                },
                velocity_uncertainty: if self.config.observe_velocity {
                    Some([self.config.velocity_uncertainty; 3])
                } else {
                    None
                },
                distance: dist,
                angle,
                confidence,
                atom_type: atom.atom_type,
            };

            readings.push(reading);
        }

        let visibility_fraction = readings.len() as f32 / atoms.len().max(1) as f32;

        let data = ObservationData {
            readings,
            observer_position: self.position,
            observer_direction: self.direction,
            config: self.config,
        };

        self.sequence += 1;
        self.last_observation_time = Some(current_time);

        let observation = Observation::new(data, current_time, self.sequence);

        PartialObservation {
            observation,
            visibility_fraction,
            occluded_ids,
            out_of_range_ids,
            outside_fov_ids,
        }
    }

    /// Check if enough time has passed for a new observation.
    pub fn can_observe(&self, current_time: f64) -> bool {
        match self.last_observation_time {
            Some(last_time) => {
                let min_interval = 1.0 / self.config.update_rate as f64;
                current_time - last_time >= min_interval
            }
            None => true,
        }
    }
}

impl Observer for PartialObserver {
    fn observe(&self, atoms: &[Atom], sim_box: &SimulationBox) -> Observation {
        // Create a temporary mutable copy for the observation
        let mut observer = self.clone();
        let partial = observer.observe_partial(atoms, sim_box, 0.0);
        partial.observation
    }

    fn position(&self) -> [f32; 3] {
        self.position
    }

    fn direction(&self) -> [f32; 3] {
        self.direction
    }

    fn config(&self) -> &ObserverConfig {
        &self.config
    }
}

/// Omniscient observer that sees everything (for testing/debugging).
#[derive(Debug, Clone)]
pub struct OmniscientObserver {
    config: ObserverConfig,
    sequence: u64,
}

impl OmniscientObserver {
    /// Create a new omniscient observer.
    pub fn new() -> Self {
        Self {
            config: ObserverConfig {
                fov_angle: std::f32::consts::PI,
                max_range: f32::MAX,
                min_range: 0.0,
                update_rate: f32::MAX,
                ..Default::default()
            },
            sequence: 0,
        }
    }
}

impl Default for OmniscientObserver {
    fn default() -> Self {
        Self::new()
    }
}

impl Observer for OmniscientObserver {
    fn observe(&self, atoms: &[Atom], _sim_box: &SimulationBox) -> Observation {
        let readings: Vec<SensorReading> = atoms
            .iter()
            .map(|atom| SensorReading {
                atom_id: atom.id,
                position: atom.position,
                position_uncertainty: [0.0; 3], // Perfect knowledge
                velocity: Some(atom.velocity),
                velocity_uncertainty: Some([0.0; 3]),
                distance: 0.0,
                angle: 0.0,
                confidence: 1.0,
                atom_type: atom.atom_type,
            })
            .collect();

        let data = ObservationData {
            readings,
            observer_position: [0.0; 3],
            observer_direction: [1.0, 0.0, 0.0],
            config: self.config,
        };

        Observation::new(data, 0.0, self.sequence)
    }

    fn position(&self) -> [f32; 3] {
        [0.0; 3]
    }

    fn direction(&self) -> [f32; 3] {
        [1.0, 0.0, 0.0]
    }

    fn config(&self) -> &ObserverConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_atoms() -> Vec<Atom> {
        vec![
            Atom::new(0, 0, 1.0).with_position(5.0, 5.0, 5.0),
            Atom::new(1, 0, 1.0).with_position(6.0, 5.0, 5.0),
            Atom::new(2, 0, 1.0).with_position(15.0, 5.0, 5.0), // Out of default range
        ]
    }

    #[test]
    fn test_partial_observer_creation() {
        let config = ObserverConfig::default();
        let observer = PartialObserver::new([0.0, 0.0, 0.0], [1.0, 0.0, 0.0], config);

        assert_eq!(observer.position(), [0.0, 0.0, 0.0]);
        assert_eq!(observer.direction(), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn test_direction_normalization() {
        let config = ObserverConfig::default();
        let observer = PartialObserver::new([0.0, 0.0, 0.0], [2.0, 0.0, 0.0], config);

        // Direction should be normalized
        let dir = observer.direction();
        let mag = (dir[0].powi(2) + dir[1].powi(2) + dir[2].powi(2)).sqrt();
        assert!((mag - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_omniscient_observer() {
        let observer = OmniscientObserver::new();
        let atoms = create_test_atoms();
        let sim_box = SimulationBox::cubic(20.0);

        let observation = observer.observe(&atoms, &sim_box);

        // Should see all atoms
        assert_eq!(observation.visible_count(), 3);
    }

    #[test]
    fn test_partial_observation_range_filtering() {
        let config = ObserverConfig {
            max_range: 10.0,
            fov_angle: std::f32::consts::PI,
            ..Default::default()
        };
        let mut observer = PartialObserver::new([0.0, 5.0, 5.0], [1.0, 0.0, 0.0], config);
        let atoms = create_test_atoms();
        // Use larger box to avoid periodic wrapping
        // Atom 2 is at [15, 5, 5], distance 15 from observer at [0, 5, 5]
        // With box size 20, minimum image would wrap 15 -> -5 (distance 5)
        // Use box size 40 so 15 < 20 (half box) and no wrapping occurs
        let sim_box = SimulationBox::cubic(40.0);

        let partial = observer.observe_partial(&atoms, &sim_box, 0.0);

        // Third atom is at distance 15, should be out of range (max_range = 10)
        assert_eq!(partial.visible_count(), 2);
        assert!(partial.out_of_range_ids.contains(&2));
    }

    #[test]
    fn test_observation_entropy_calculation() {
        let observer = OmniscientObserver::new();
        let atoms = create_test_atoms();
        let sim_box = SimulationBox::cubic(20.0);

        let observation = observer.observe(&atoms, &sim_box);

        // Entropy should be non-negative
        assert!(observation.entropy >= 0.0);
    }

    #[test]
    fn test_sensor_reading_confidence() {
        let reading = SensorReading {
            atom_id: 0,
            position: [0.0; 3],
            position_uncertainty: [0.1; 3],
            velocity: None,
            velocity_uncertainty: None,
            distance: 5.0,
            angle: 0.0,
            confidence: 0.8,
            atom_type: 0,
        };

        assert!(reading.is_reliable(0.5));
        assert!(!reading.is_reliable(0.9));
    }
}
