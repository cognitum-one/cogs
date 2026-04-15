//! Sensor noise models for perception.
//!
//! This module provides realistic noise models for simulating imperfect
//! sensors, including Gaussian noise, occlusion, and depth-dependent
//! uncertainty.
//!
//! # Overview
//!
//! Real sensors have various sources of noise and uncertainty:
//!
//! - **Gaussian noise**: Random measurement errors
//! - **Occlusion**: Objects blocking line of sight
//! - **Depth noise**: Range-dependent uncertainty
//!
//! # Examples
//!
//! ```rust,no_run
//! use fxnn::perception::{GaussianNoise, OcclusionModel, DepthNoise, NoiseModel};
//!
//! // Add Gaussian noise to observations
//! let gaussian = GaussianNoise::new(0.1); // 0.1 sigma
//!
//! // Check for occlusions
//! let occlusion = OcclusionModel::new(0.5); // 0.5 unit radius objects
//!
//! // Range-dependent depth noise
//! let depth_noise = DepthNoise::new(0.01, 0.001); // base + range-dependent
//! ```

use super::observer::{ObservationData, SensorReading};
use crate::types::SimulationBox;
use rand::{Rng, SeedableRng};
use rand_distr::{Distribution, Normal};
use rand_xoshiro::Xoshiro256PlusPlus;
use serde::{Deserialize, Serialize};

/// Trait for noise models that can be applied to observations.
pub trait NoiseModel {
    /// Apply noise to observation data.
    ///
    /// # Arguments
    ///
    /// * `data` - The observation data to add noise to
    ///
    /// # Returns
    ///
    /// Modified observation data with noise applied.
    fn apply(&self, data: &ObservationData) -> ObservationData;

    /// Get the expected noise magnitude.
    fn noise_magnitude(&self) -> f32;
}

/// Gaussian (additive) noise model.
///
/// Adds zero-mean Gaussian noise to position and optionally velocity
/// measurements, simulating random measurement errors.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GaussianNoise {
    /// Standard deviation of position noise.
    position_sigma: f32,

    /// Standard deviation of velocity noise.
    velocity_sigma: f32,

    /// Whether to apply noise to velocity.
    apply_to_velocity: bool,

    /// Random seed for reproducibility.
    seed: u64,
}

impl GaussianNoise {
    /// Create a new Gaussian noise model.
    ///
    /// # Arguments
    ///
    /// * `position_sigma` - Standard deviation for position noise
    pub fn new(position_sigma: f32) -> Self {
        Self {
            position_sigma,
            velocity_sigma: position_sigma * 0.1,
            apply_to_velocity: false,
            seed: 42,
        }
    }

    /// Set velocity noise parameters.
    pub fn with_velocity_noise(mut self, sigma: f32) -> Self {
        self.velocity_sigma = sigma;
        self.apply_to_velocity = true;
        self
    }

    /// Set random seed for reproducibility.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Apply noise to a single position value.
    fn add_position_noise(&self, value: f32, rng: &mut impl Rng) -> f32 {
        let normal = Normal::new(0.0, self.position_sigma as f64).unwrap();
        value + normal.sample(rng) as f32
    }

    /// Apply noise to a single velocity value.
    fn add_velocity_noise(&self, value: f32, rng: &mut impl Rng) -> f32 {
        let normal = Normal::new(0.0, self.velocity_sigma as f64).unwrap();
        value + normal.sample(rng) as f32
    }
}

impl NoiseModel for GaussianNoise {
    fn apply(&self, data: &ObservationData) -> ObservationData {
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(self.seed);

        let readings = data
            .readings
            .iter()
            .map(|r| {
                let mut reading = r.clone();

                // Add position noise
                reading.position = [
                    self.add_position_noise(r.position[0], &mut rng),
                    self.add_position_noise(r.position[1], &mut rng),
                    self.add_position_noise(r.position[2], &mut rng),
                ];

                // Update uncertainty to reflect noise
                reading.position_uncertainty = [
                    (r.position_uncertainty[0].powi(2) + self.position_sigma.powi(2)).sqrt(),
                    (r.position_uncertainty[1].powi(2) + self.position_sigma.powi(2)).sqrt(),
                    (r.position_uncertainty[2].powi(2) + self.position_sigma.powi(2)).sqrt(),
                ];

                // Optionally add velocity noise
                if self.apply_to_velocity {
                    if let Some(v) = r.velocity {
                        reading.velocity = Some([
                            self.add_velocity_noise(v[0], &mut rng),
                            self.add_velocity_noise(v[1], &mut rng),
                            self.add_velocity_noise(v[2], &mut rng),
                        ]);

                        if let Some(vu) = r.velocity_uncertainty {
                            reading.velocity_uncertainty = Some([
                                (vu[0].powi(2) + self.velocity_sigma.powi(2)).sqrt(),
                                (vu[1].powi(2) + self.velocity_sigma.powi(2)).sqrt(),
                                (vu[2].powi(2) + self.velocity_sigma.powi(2)).sqrt(),
                            ]);
                        }
                    }
                }

                reading
            })
            .collect();

        ObservationData {
            readings,
            observer_position: data.observer_position,
            observer_direction: data.observer_direction,
            config: data.config,
        }
    }

    fn noise_magnitude(&self) -> f32 {
        self.position_sigma
    }
}

/// Result of occlusion testing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OcclusionResult {
    /// Whether the target is occluded.
    pub is_occluded: bool,

    /// ID of the occluding object (if any).
    pub occluder_id: Option<u32>,

    /// Distance to the occluder (if any).
    pub occluder_distance: Option<f32>,

    /// Partial visibility factor [0, 1] (1 = fully visible).
    pub visibility: f32,
}

impl OcclusionResult {
    /// Create a result indicating full visibility.
    pub fn visible() -> Self {
        Self {
            is_occluded: false,
            occluder_id: None,
            occluder_distance: None,
            visibility: 1.0,
        }
    }

    /// Create a result indicating full occlusion.
    pub fn occluded(occluder_id: u32, distance: f32) -> Self {
        Self {
            is_occluded: true,
            occluder_id: Some(occluder_id),
            occluder_distance: Some(distance),
            visibility: 0.0,
        }
    }

    /// Create a result indicating partial occlusion.
    pub fn partial(visibility: f32, occluder_id: Option<u32>) -> Self {
        Self {
            is_occluded: visibility < 0.5,
            occluder_id,
            occluder_distance: None,
            visibility,
        }
    }
}

/// Ray-based occlusion model.
///
/// Tests line-of-sight from the observer to each observation target,
/// checking if other objects block the view.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcclusionModel {
    /// Radius of objects for occlusion testing.
    object_radius: f32,

    /// Minimum visibility to consider an object visible.
    min_visibility: f32,

    /// Whether to compute partial occlusions.
    partial_occlusion: bool,
}

impl OcclusionModel {
    /// Create a new occlusion model.
    ///
    /// # Arguments
    ///
    /// * `object_radius` - Radius of objects for ray-sphere intersection
    pub fn new(object_radius: f32) -> Self {
        Self {
            object_radius,
            min_visibility: 0.1,
            partial_occlusion: true,
        }
    }

    /// Set minimum visibility threshold.
    pub fn with_min_visibility(mut self, threshold: f32) -> Self {
        self.min_visibility = threshold;
        self
    }

    /// Disable partial occlusion computation.
    pub fn without_partial_occlusion(mut self) -> Self {
        self.partial_occlusion = false;
        self
    }

    /// Test if a ray from origin to target intersects a sphere.
    ///
    /// Uses the geometric method: project sphere center onto ray,
    /// check if closest point is within sphere radius.
    fn ray_sphere_intersection(
        &self,
        ray_origin: [f32; 3],
        ray_dir: [f32; 3],
        ray_length: f32,
        sphere_center: [f32; 3],
    ) -> Option<f32> {
        // Vector from ray origin to sphere center
        let oc = [
            sphere_center[0] - ray_origin[0],
            sphere_center[1] - ray_origin[1],
            sphere_center[2] - ray_origin[2],
        ];

        // Project onto ray direction
        let t = oc[0] * ray_dir[0] + oc[1] * ray_dir[1] + oc[2] * ray_dir[2];

        // Check if closest point is behind ray or beyond target
        if t < 0.0 || t > ray_length {
            return None;
        }

        // Calculate closest point on ray to sphere center
        let closest = [
            ray_origin[0] + t * ray_dir[0],
            ray_origin[1] + t * ray_dir[1],
            ray_origin[2] + t * ray_dir[2],
        ];

        // Distance from closest point to sphere center
        let d2 = (closest[0] - sphere_center[0]).powi(2)
            + (closest[1] - sphere_center[1]).powi(2)
            + (closest[2] - sphere_center[2]).powi(2);

        if d2 <= self.object_radius.powi(2) {
            Some(t)
        } else {
            None
        }
    }

    /// Test occlusion for a single reading against all other readings.
    pub fn test_occlusion(
        &self,
        observer_pos: [f32; 3],
        target: &SensorReading,
        potential_occluders: &[SensorReading],
        sim_box: &SimulationBox,
    ) -> OcclusionResult {
        // Calculate ray from observer to target
        let disp = sim_box.displacement(&observer_pos, &target.position);
        let dist = (disp[0].powi(2) + disp[1].powi(2) + disp[2].powi(2)).sqrt();

        if dist < 1e-6 {
            return OcclusionResult::visible();
        }

        let ray_dir = [disp[0] / dist, disp[1] / dist, disp[2] / dist];

        let mut closest_occluder: Option<(u32, f32)> = None;
        let mut total_occlusion = 0.0;

        for occluder in potential_occluders {
            // Skip self
            if occluder.atom_id == target.atom_id {
                continue;
            }

            // Test ray-sphere intersection
            if let Some(hit_dist) =
                self.ray_sphere_intersection(observer_pos, ray_dir, dist, occluder.position)
            {
                // Found an occluder
                if self.partial_occlusion {
                    // Approximate partial occlusion based on how centered the occluder is
                    let occlusion_amount = 0.5; // Simplified
                    total_occlusion += occlusion_amount;
                }

                if closest_occluder.is_none()
                    || hit_dist < closest_occluder.as_ref().unwrap().1
                {
                    closest_occluder = Some((occluder.atom_id, hit_dist));
                }
            }
        }

        if let Some((id, hit_dist)) = closest_occluder {
            if self.partial_occlusion {
                let visibility = (1.0_f32 - total_occlusion).max(0.0);
                if visibility >= self.min_visibility {
                    OcclusionResult::partial(visibility, Some(id))
                } else {
                    OcclusionResult::occluded(id, hit_dist)
                }
            } else {
                OcclusionResult::occluded(id, hit_dist)
            }
        } else {
            OcclusionResult::visible()
        }
    }

    /// Apply occlusion filtering to observation data.
    pub fn filter_occluded(&self, data: &ObservationData, sim_box: &SimulationBox) -> ObservationData {
        let readings: Vec<SensorReading> = data
            .readings
            .iter()
            .filter_map(|reading| {
                let result = self.test_occlusion(
                    data.observer_position,
                    reading,
                    &data.readings,
                    sim_box,
                );

                if result.visibility >= self.min_visibility {
                    let mut modified = reading.clone();
                    // Reduce confidence based on visibility
                    modified.confidence *= result.visibility;
                    Some(modified)
                } else {
                    None
                }
            })
            .collect();

        ObservationData {
            readings,
            observer_position: data.observer_position,
            observer_direction: data.observer_direction,
            config: data.config,
        }
    }
}

/// Range-dependent depth noise model.
///
/// Simulates how measurement uncertainty increases with distance,
/// typical of lidar, radar, and visual depth sensors.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DepthNoise {
    /// Base noise (constant component).
    base_sigma: f32,

    /// Range-dependent noise coefficient.
    range_coefficient: f32,

    /// Quadratic range coefficient (for long-range degradation).
    range_squared_coefficient: f32,

    /// Maximum noise cap.
    max_noise: f32,

    /// Random seed.
    seed: u64,
}

impl DepthNoise {
    /// Create a new depth noise model.
    ///
    /// # Arguments
    ///
    /// * `base_sigma` - Constant noise component
    /// * `range_coefficient` - Linear range-dependent coefficient
    pub fn new(base_sigma: f32, range_coefficient: f32) -> Self {
        Self {
            base_sigma,
            range_coefficient,
            range_squared_coefficient: 0.0,
            max_noise: f32::MAX,
            seed: 42,
        }
    }

    /// Add quadratic range dependence.
    pub fn with_quadratic(mut self, coefficient: f32) -> Self {
        self.range_squared_coefficient = coefficient;
        self
    }

    /// Set maximum noise cap.
    pub fn with_max_noise(mut self, max: f32) -> Self {
        self.max_noise = max;
        self
    }

    /// Set random seed.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Calculate noise sigma for a given distance.
    pub fn sigma_at_distance(&self, distance: f32) -> f32 {
        let sigma = self.base_sigma
            + self.range_coefficient * distance
            + self.range_squared_coefficient * distance.powi(2);
        sigma.min(self.max_noise)
    }
}

impl NoiseModel for DepthNoise {
    fn apply(&self, data: &ObservationData) -> ObservationData {
        let mut rng = Xoshiro256PlusPlus::seed_from_u64(self.seed);

        let readings = data
            .readings
            .iter()
            .map(|r| {
                let sigma = self.sigma_at_distance(r.distance);
                let normal = Normal::new(0.0, sigma as f64).unwrap();

                let mut reading = r.clone();

                // Apply range-dependent noise along the radial direction
                let noise = normal.sample(&mut rng) as f32;

                // Get unit vector from observer to target
                let disp = [
                    r.position[0] - data.observer_position[0],
                    r.position[1] - data.observer_position[1],
                    r.position[2] - data.observer_position[2],
                ];

                if r.distance > 1e-6 {
                    let unit = [
                        disp[0] / r.distance,
                        disp[1] / r.distance,
                        disp[2] / r.distance,
                    ];

                    // Add noise in radial direction
                    reading.position = [
                        r.position[0] + noise * unit[0],
                        r.position[1] + noise * unit[1],
                        r.position[2] + noise * unit[2],
                    ];
                }

                // Update uncertainty
                reading.position_uncertainty = [
                    (r.position_uncertainty[0].powi(2) + sigma.powi(2)).sqrt(),
                    (r.position_uncertainty[1].powi(2) + sigma.powi(2)).sqrt(),
                    (r.position_uncertainty[2].powi(2) + sigma.powi(2)).sqrt(),
                ];

                // Reduce confidence based on distance
                let confidence_reduction = 1.0 / (1.0 + sigma / self.base_sigma);
                reading.confidence *= confidence_reduction;

                reading
            })
            .collect();

        ObservationData {
            readings,
            observer_position: data.observer_position,
            observer_direction: data.observer_direction,
            config: data.config,
        }
    }

    fn noise_magnitude(&self) -> f32 {
        self.base_sigma
    }
}

/// Combined noise model that chains multiple noise sources.
#[derive(Debug, Default)]
pub struct CompositeNoise {
    /// Ordered list of noise models to apply.
    models: Vec<Box<dyn NoiseModelDyn>>,
}

/// Dynamic dispatch wrapper for noise models.
pub trait NoiseModelDyn: std::fmt::Debug + Send + Sync {
    /// Apply noise to observation data.
    fn apply_dyn(&self, data: &ObservationData) -> ObservationData;

    /// Clone the noise model.
    fn clone_box(&self) -> Box<dyn NoiseModelDyn>;
}

impl<T: NoiseModel + Clone + std::fmt::Debug + Send + Sync + 'static> NoiseModelDyn for T {
    fn apply_dyn(&self, data: &ObservationData) -> ObservationData {
        self.apply(data)
    }

    fn clone_box(&self) -> Box<dyn NoiseModelDyn> {
        Box::new(self.clone())
    }
}

impl Clone for CompositeNoise {
    fn clone(&self) -> Self {
        Self {
            models: self.models.iter().map(|m| m.clone_box()).collect(),
        }
    }
}

impl CompositeNoise {
    /// Create an empty composite noise model.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a noise model to the chain.
    pub fn add<N: NoiseModel + Clone + std::fmt::Debug + Send + Sync + 'static>(
        mut self,
        model: N,
    ) -> Self {
        self.models.push(Box::new(model));
        self
    }

    /// Apply all noise models in sequence.
    pub fn apply(&self, data: &ObservationData) -> ObservationData {
        let mut result = data.clone();
        for model in &self.models {
            result = model.apply_dyn(&result);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::perception::observer::ObserverConfig;

    fn create_test_data() -> ObservationData {
        let readings = vec![
            SensorReading {
                atom_id: 0,
                position: [5.0, 0.0, 0.0],
                position_uncertainty: [0.1; 3],
                velocity: Some([1.0, 0.0, 0.0]),
                velocity_uncertainty: Some([0.1; 3]),
                distance: 5.0,
                angle: 0.0,
                confidence: 0.9,
                atom_type: 0,
            },
            SensorReading {
                atom_id: 1,
                position: [10.0, 0.0, 0.0],
                position_uncertainty: [0.1; 3],
                velocity: Some([0.1, 0.0, 0.0]),
                velocity_uncertainty: Some([0.1; 3]),
                distance: 10.0,
                angle: 0.0,
                confidence: 0.8,
                atom_type: 0,
            },
        ];

        ObservationData {
            readings,
            observer_position: [0.0; 3],
            observer_direction: [1.0, 0.0, 0.0],
            config: ObserverConfig::default(),
        }
    }

    #[test]
    fn test_gaussian_noise() {
        let noise = GaussianNoise::new(0.1).with_seed(42);
        let data = create_test_data();

        let noisy = noise.apply(&data);

        // Positions should be different
        assert_ne!(noisy.readings[0].position, data.readings[0].position);

        // Uncertainty should increase
        assert!(
            noisy.readings[0].position_uncertainty[0] > data.readings[0].position_uncertainty[0]
        );
    }

    #[test]
    fn test_depth_noise() {
        let noise = DepthNoise::new(0.01, 0.001);
        let _data = create_test_data();

        // Far reading should have more noise
        let sigma_near = noise.sigma_at_distance(5.0);
        let sigma_far = noise.sigma_at_distance(10.0);

        assert!(sigma_far > sigma_near);
    }

    #[test]
    fn test_occlusion_result() {
        let visible = OcclusionResult::visible();
        assert!(!visible.is_occluded);
        assert_eq!(visible.visibility, 1.0);

        let occluded = OcclusionResult::occluded(1, 5.0);
        assert!(occluded.is_occluded);
        assert_eq!(occluded.visibility, 0.0);
    }

    #[test]
    fn test_composite_noise() {
        let composite = CompositeNoise::new()
            .add(GaussianNoise::new(0.1).with_seed(42))
            .add(DepthNoise::new(0.01, 0.001).with_seed(42));

        let data = create_test_data();
        let noisy = composite.apply(&data);

        // Should have more uncertainty than single noise source
        assert!(noisy.readings[0].position_uncertainty[0] > 0.1);
    }

    #[test]
    fn test_occlusion_model_ray_test() {
        // Use a larger object radius to ensure clear occlusion detection
        let model = OcclusionModel::new(2.0).without_partial_occlusion();

        // Create occluder between observer and target with clear geometry
        let observer_pos = [0.0, 0.0, 0.0];
        let target = SensorReading {
            atom_id: 0,
            position: [10.0, 0.0, 0.0],
            position_uncertainty: [0.1; 3],
            velocity: None,
            velocity_uncertainty: None,
            distance: 10.0,
            angle: 0.0,
            confidence: 0.9,
            atom_type: 0,
        };

        // Occluder at position [5.0, 0.0, 0.0] - directly on the ray
        let occluder = SensorReading {
            atom_id: 1,
            position: [5.0, 0.0, 0.0], // Directly between observer and target
            position_uncertainty: [0.1; 3],
            velocity: None,
            velocity_uncertainty: None,
            distance: 5.0,
            angle: 0.0,
            confidence: 0.9,
            atom_type: 0,
        };

        let sim_box = SimulationBox::cubic(30.0); // Larger box to avoid PBC issues
        let result = model.test_occlusion(observer_pos, &target, &[occluder], &sim_box);

        // Either fully occluded or partially occluded (visibility < 1.0)
        assert!(
            result.is_occluded || result.visibility < 1.0,
            "Expected occlusion: is_occluded={}, visibility={}",
            result.is_occluded, result.visibility
        );
    }
}
