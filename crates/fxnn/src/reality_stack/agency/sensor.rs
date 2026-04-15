//! Sensors for agent perception

use crate::types::{Atom, SimulationBox};
use serde::{Deserialize, Serialize};

/// Unique identifier for sensors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SensorId(pub u32);

/// Kind of sensor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SensorKind {
    /// Distance sensor (detects nearby atoms)
    Distance {
        range: f32,
        field_of_view: f32, // radians
    },
    /// Force sensor (detects forces on controlled atoms)
    Force {
        sensitivity: f32,
    },
    /// Chemical sensor (detects atom types/charges)
    Chemical {
        range: f32,
        specificity: Vec<u16>, // atom types to detect
    },
    /// Velocity sensor (detects velocities)
    Velocity {
        range: f32,
    },
    /// Temperature sensor (local thermal sensing)
    Temperature {
        range: f32,
    },
    /// Proprioceptive sensor (senses own state)
    Proprioceptive,
}

/// Reading from a sensor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorReading {
    /// Sensor that produced this reading
    pub sensor_id: SensorId,
    /// Sensor kind
    pub kind: SensorKind,
    /// Raw values from the sensor
    pub values: Vec<f32>,
    /// Timestamp of reading
    pub timestamp: u64,
    /// Noise level in this reading
    pub noise_level: f32,
}

/// Trait for sensor implementations
pub trait Sensor: Send + Sync {
    /// Get sensor ID
    fn id(&self) -> SensorId;

    /// Get sensor kind
    fn kind(&self) -> SensorKind;

    /// Get sensor name
    fn name(&self) -> &str;

    /// Read sensor values from world state
    fn read(
        &self,
        agent_position: [f32; 3],
        atoms: &[Atom],
        box_: &SimulationBox,
    ) -> SensorReading;

    /// Get bandwidth (number of floats per reading)
    fn bandwidth(&self) -> usize;

    /// Apply noise to reading
    fn apply_noise(&self, reading: &mut SensorReading, noise_level: f32);
}

/// Distance sensor for detecting nearby atoms
pub struct DistanceSensor {
    id: SensorId,
    name: String,
    range: f32,
    field_of_view: f32,
    max_detections: usize,
}

impl DistanceSensor {
    /// Create a new distance sensor
    pub fn new(range: f32) -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);

        Self {
            id: SensorId(COUNTER.fetch_add(1, Ordering::SeqCst)),
            name: "DistanceSensor".to_string(),
            range,
            field_of_view: std::f32::consts::PI * 2.0, // Full sphere
            max_detections: 16,
        }
    }

    /// Set field of view
    pub fn with_fov(mut self, fov: f32) -> Self {
        self.field_of_view = fov;
        self
    }

    /// Set maximum detections
    pub fn with_max_detections(mut self, max: usize) -> Self {
        self.max_detections = max;
        self
    }
}

impl Sensor for DistanceSensor {
    fn id(&self) -> SensorId {
        self.id
    }

    fn kind(&self) -> SensorKind {
        SensorKind::Distance {
            range: self.range,
            field_of_view: self.field_of_view,
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn read(
        &self,
        agent_position: [f32; 3],
        atoms: &[Atom],
        box_: &SimulationBox,
    ) -> SensorReading {
        let mut values = Vec::with_capacity(self.max_detections * 4); // distance + direction per atom

        // Find atoms within range
        let mut detected: Vec<(f32, [f32; 3])> = Vec::new();

        for atom in atoms {
            let dr = box_.minimum_image(
                atom.position[0] - agent_position[0],
                atom.position[1] - agent_position[1],
                atom.position[2] - agent_position[2],
            );
            let dist_sq = dr[0] * dr[0] + dr[1] * dr[1] + dr[2] * dr[2];
            let dist = dist_sq.sqrt();

            if dist < self.range && dist > 0.01 {
                let direction = [dr[0] / dist, dr[1] / dist, dr[2] / dist];
                detected.push((dist, direction));
            }
        }

        // Sort by distance and take top N
        detected.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        detected.truncate(self.max_detections);

        // Pack into values: [dist, dx, dy, dz, ...]
        for (dist, dir) in detected {
            values.push(dist);
            values.push(dir[0]);
            values.push(dir[1]);
            values.push(dir[2]);
        }

        // Pad to fixed size
        while values.len() < self.max_detections * 4 {
            values.push(0.0);
        }

        SensorReading {
            sensor_id: self.id,
            kind: self.kind().clone(),
            values,
            timestamp: 0,
            noise_level: 0.0,
        }
    }

    fn bandwidth(&self) -> usize {
        self.max_detections * 4
    }

    fn apply_noise(&self, reading: &mut SensorReading, noise_level: f32) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for value in &mut reading.values {
            if *value != 0.0 {
                *value += rng.gen_range(-noise_level..noise_level);
            }
        }
        reading.noise_level = noise_level;
    }
}

/// Force sensor for detecting forces
pub struct ForceSensor {
    id: SensorId,
    name: String,
    sensitivity: f32,
}

impl ForceSensor {
    /// Create a new force sensor
    pub fn new(sensitivity: f32) -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);

        Self {
            id: SensorId(COUNTER.fetch_add(1, Ordering::SeqCst)),
            name: "ForceSensor".to_string(),
            sensitivity,
        }
    }
}

impl Sensor for ForceSensor {
    fn id(&self) -> SensorId {
        self.id
    }

    fn kind(&self) -> SensorKind {
        SensorKind::Force {
            sensitivity: self.sensitivity,
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn read(
        &self,
        agent_position: [f32; 3],
        atoms: &[Atom],
        _box_: &SimulationBox,
    ) -> SensorReading {
        // Aggregate forces from nearby atoms
        let mut total_force = [0.0f32; 3];

        for atom in atoms {
            let dr = [
                atom.position[0] - agent_position[0],
                atom.position[1] - agent_position[1],
                atom.position[2] - agent_position[2],
            ];
            let dist_sq = dr[0] * dr[0] + dr[1] * dr[1] + dr[2] * dr[2];

            if dist_sq < 4.0 { // Within 2 units
                total_force[0] += atom.force[0];
                total_force[1] += atom.force[1];
                total_force[2] += atom.force[2];
            }
        }

        // Apply sensitivity threshold
        let magnitude = (total_force[0].powi(2) + total_force[1].powi(2) + total_force[2].powi(2)).sqrt();
        let values = if magnitude > self.sensitivity {
            vec![total_force[0], total_force[1], total_force[2], magnitude]
        } else {
            vec![0.0, 0.0, 0.0, 0.0]
        };

        SensorReading {
            sensor_id: self.id,
            kind: self.kind().clone(),
            values,
            timestamp: 0,
            noise_level: 0.0,
        }
    }

    fn bandwidth(&self) -> usize {
        4 // fx, fy, fz, magnitude
    }

    fn apply_noise(&self, reading: &mut SensorReading, noise_level: f32) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for value in &mut reading.values {
            *value += rng.gen_range(-noise_level..noise_level);
        }
        reading.noise_level = noise_level;
    }
}

/// Chemical sensor for detecting atom types
pub struct ChemicalSensor {
    id: SensorId,
    name: String,
    range: f32,
    target_types: Vec<u16>,
}

impl ChemicalSensor {
    /// Create a new chemical sensor
    pub fn new(range: f32, target_types: Vec<u16>) -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);

        Self {
            id: SensorId(COUNTER.fetch_add(1, Ordering::SeqCst)),
            name: "ChemicalSensor".to_string(),
            range,
            target_types,
        }
    }
}

impl Sensor for ChemicalSensor {
    fn id(&self) -> SensorId {
        self.id
    }

    fn kind(&self) -> SensorKind {
        SensorKind::Chemical {
            range: self.range,
            specificity: self.target_types.clone(),
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn read(
        &self,
        agent_position: [f32; 3],
        atoms: &[Atom],
        box_: &SimulationBox,
    ) -> SensorReading {
        let mut concentrations: Vec<f32> = vec![0.0; self.target_types.len()];

        for atom in atoms {
            let dr = box_.minimum_image(
                atom.position[0] - agent_position[0],
                atom.position[1] - agent_position[1],
                atom.position[2] - agent_position[2],
            );
            let dist = (dr[0] * dr[0] + dr[1] * dr[1] + dr[2] * dr[2]).sqrt();

            if dist < self.range {
                if let Some(idx) = self.target_types.iter().position(|&t| t == atom.atom_type) {
                    // Concentration falls off with distance
                    concentrations[idx] += 1.0 / (1.0 + dist);
                }
            }
        }

        SensorReading {
            sensor_id: self.id,
            kind: self.kind().clone(),
            values: concentrations,
            timestamp: 0,
            noise_level: 0.0,
        }
    }

    fn bandwidth(&self) -> usize {
        self.target_types.len()
    }

    fn apply_noise(&self, reading: &mut SensorReading, noise_level: f32) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for value in &mut reading.values {
            *value = (*value + rng.gen_range(-noise_level..noise_level)).max(0.0);
        }
        reading.noise_level = noise_level;
    }
}
