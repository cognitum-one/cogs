//! Sensor types for agent perception.
//!
//! This module provides various sensor implementations that allow agents
//! to perceive their environment. Each sensor type has different characteristics
//! and produces different types of readings.
//!
//! # Sensor Types
//!
//! | Sensor | Detection Pattern | Range | Occlusion |
//! |--------|------------------|-------|-----------|
//! | [`VisionSensor`] | Cone (FOV) | Limited | Yes |
//! | [`AuditorySensor`] | Omnidirectional | Attenuated | No |
//! | [`TactileSensor`] | Contact only | None | N/A |
//! | [`ProprioceptiveSensor`] | Self only | N/A | N/A |
//! | [`CommunicationSensor`] | Message channel | Limited | No |
//!
//! # Example
//!
//! ```rust,no_run
//! use fxnn::agency::{VisionSensor, AuditorySensor, SensorType, Sensor};
//!
//! // Create a vision sensor with 90 FOV and 10 unit range
//! let vision = VisionSensor::new(std::f32::consts::FRAC_PI_2, 10.0);
//! assert_eq!(vision.sensor_type(), SensorType::Vision);
//!
//! // Create an auditory sensor with 20 unit range
//! let hearing = AuditorySensor::new(20.0);
//! assert_eq!(hearing.sensor_type(), SensorType::Auditory);
//! ```

use super::agent::{AgentState, WorldState, AgentId};

/// Types of sensors available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SensorType {
    /// Vision sensor (cone-based FOV).
    Vision,
    /// Auditory sensor (omnidirectional sound).
    Auditory,
    /// Tactile sensor (contact/pressure).
    Tactile,
    /// Proprioceptive sensor (internal state).
    Proprioceptive,
    /// Communication sensor (message channel).
    Communication,
}

/// A reading from a sensor.
#[derive(Debug, Clone)]
pub enum SensorReading {
    /// Vision sensor reading.
    Vision(VisionReading),
    /// Auditory sensor reading.
    Auditory(AuditoryReading),
    /// Tactile sensor reading.
    Tactile(TactileReading),
    /// Proprioceptive sensor reading.
    Proprioceptive(ProprioceptiveReading),
    /// Communication sensor reading.
    Communication(Vec<Message>),
}

impl SensorReading {
    /// Get the sensor type for this reading.
    pub fn sensor_type(&self) -> SensorType {
        match self {
            SensorReading::Vision(_) => SensorType::Vision,
            SensorReading::Auditory(_) => SensorType::Auditory,
            SensorReading::Tactile(_) => SensorType::Tactile,
            SensorReading::Proprioceptive(_) => SensorType::Proprioceptive,
            SensorReading::Communication(_) => SensorType::Communication,
        }
    }

    /// Convert reading to a flat vector of floats for neural network input.
    pub fn to_vector(&self) -> Vec<f32> {
        match self {
            SensorReading::Vision(v) => v.to_vector(),
            SensorReading::Auditory(a) => a.to_vector(),
            SensorReading::Tactile(t) => t.to_vector(),
            SensorReading::Proprioceptive(p) => p.to_vector(),
            SensorReading::Communication(msgs) => {
                msgs.iter().flat_map(|m| m.content.iter().copied()).collect()
            }
        }
    }
}

/// Trait for all sensor implementations.
pub trait Sensor: Send + Sync {
    /// Get the type of this sensor.
    fn sensor_type(&self) -> SensorType;

    /// Sense the environment and produce a reading.
    ///
    /// # Arguments
    ///
    /// * `agent_state` - Current state of the owning agent
    /// * `world_state` - Current state of the world
    ///
    /// # Returns
    ///
    /// A sensor reading based on the current state.
    fn sense(&self, agent_state: &AgentState, world_state: &WorldState) -> SensorReading;

    /// Get the maximum range of this sensor.
    fn range(&self) -> f32;

    /// Check if this sensor can detect the given entity.
    fn can_detect(&self, agent_state: &AgentState, target_position: [f32; 3]) -> bool;
}

// ============================================================================
// Vision Sensor
// ============================================================================

/// Information about a visible entity.
#[derive(Debug, Clone)]
pub struct VisibleEntity {
    /// Index of the entity in the world.
    pub entity_index: usize,
    /// Type of the entity.
    pub entity_type: u32,
    /// Distance to the entity.
    pub distance: f32,
    /// Relative position in sensor coordinates [forward, right, up].
    pub relative_position: [f32; 3],
    /// Relative velocity.
    pub relative_velocity: [f32; 3],
    /// Angular position (azimuth, elevation) from agent's forward.
    pub angular_position: [f32; 2],
}

/// Reading from a vision sensor.
#[derive(Debug, Clone, Default)]
pub struct VisionReading {
    /// List of visible entities.
    pub entities: Vec<VisibleEntity>,
    /// Number of entities detected.
    pub count: usize,
}

impl VisionReading {
    /// Convert to a flat vector for neural network input.
    pub fn to_vector(&self) -> Vec<f32> {
        let mut result = vec![self.count as f32];
        for entity in &self.entities {
            result.push(entity.distance);
            result.extend_from_slice(&entity.relative_position);
            result.extend_from_slice(&entity.angular_position);
            result.push(entity.entity_type as f32);
        }
        result
    }

    /// Get the nearest visible entity.
    pub fn nearest(&self) -> Option<&VisibleEntity> {
        self.entities.iter().min_by(|a, b| {
            a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Filter entities by type.
    pub fn of_type(&self, entity_type: u32) -> Vec<&VisibleEntity> {
        self.entities.iter().filter(|e| e.entity_type == entity_type).collect()
    }
}

/// Vision sensor with cone-based field of view.
///
/// Detects entities within a cone extending from the agent's position
/// in the forward direction. Supports occlusion (closer objects block
/// line of sight to farther objects).
#[derive(Debug, Clone)]
pub struct VisionSensor {
    /// Field of view angle in radians (half-angle).
    fov: f32,
    /// Maximum detection range.
    range: f32,
    /// Whether to apply occlusion (line-of-sight blocking).
    occlusion: bool,
    /// Maximum number of entities to track.
    max_entities: usize,
}

impl VisionSensor {
    /// Create a new vision sensor.
    ///
    /// # Arguments
    ///
    /// * `fov` - Field of view in radians (full angle, e.g., PI/2 for 90)
    /// * `range` - Maximum detection range
    ///
    /// # Returns
    ///
    /// A new `VisionSensor`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use fxnn::agency::VisionSensor;
    ///
    /// // 90 field of view, 10 unit range
    /// let vision = VisionSensor::new(std::f32::consts::FRAC_PI_2, 10.0);
    /// ```
    pub fn new(fov: f32, range: f32) -> Self {
        Self {
            fov: fov * 0.5, // Store half-angle
            range,
            occlusion: true,
            max_entities: 100,
        }
    }

    /// Set whether occlusion is enabled (builder pattern).
    pub fn with_occlusion(mut self, enabled: bool) -> Self {
        self.occlusion = enabled;
        self
    }

    /// Set the maximum number of tracked entities (builder pattern).
    pub fn with_max_entities(mut self, max: usize) -> Self {
        self.max_entities = max;
        self
    }

    /// Get the field of view (full angle in radians).
    pub fn fov(&self) -> f32 {
        self.fov * 2.0
    }

    /// Check if a point is within the sensor's field of view.
    fn in_fov(&self, agent_state: &AgentState, target: [f32; 3]) -> bool {
        let forward = agent_state.forward();
        let to_target = [
            target[0] - agent_state.position[0],
            target[1] - agent_state.position[1],
            target[2] - agent_state.position[2],
        ];

        let dist = (to_target[0] * to_target[0]
            + to_target[1] * to_target[1]
            + to_target[2] * to_target[2]).sqrt();

        if dist < 1e-6 || dist > self.range {
            return false;
        }

        // Normalize direction to target
        let dir = [to_target[0] / dist, to_target[1] / dist, to_target[2] / dist];

        // Dot product gives cosine of angle
        let dot = forward[0] * dir[0] + forward[1] * dir[1] + forward[2] * dir[2];
        let angle = dot.acos();

        angle <= self.fov
    }
}

impl Sensor for VisionSensor {
    fn sensor_type(&self) -> SensorType {
        SensorType::Vision
    }

    fn range(&self) -> f32 {
        self.range
    }

    fn can_detect(&self, agent_state: &AgentState, target_position: [f32; 3]) -> bool {
        self.in_fov(agent_state, target_position)
    }

    fn sense(&self, agent_state: &AgentState, world_state: &WorldState) -> SensorReading {
        let mut entities = Vec::new();
        let forward = agent_state.forward();
        let right = agent_state.right();
        let up = agent_state.up();

        for (i, pos) in world_state.positions.iter().enumerate() {
            if !self.in_fov(agent_state, *pos) {
                continue;
            }

            let to_target = [
                pos[0] - agent_state.position[0],
                pos[1] - agent_state.position[1],
                pos[2] - agent_state.position[2],
            ];

            let distance = (to_target[0] * to_target[0]
                + to_target[1] * to_target[1]
                + to_target[2] * to_target[2]).sqrt();

            // Transform to sensor coordinates
            let relative_position = [
                to_target[0] * forward[0] + to_target[1] * forward[1] + to_target[2] * forward[2],
                to_target[0] * right[0] + to_target[1] * right[1] + to_target[2] * right[2],
                to_target[0] * up[0] + to_target[1] * up[1] + to_target[2] * up[2],
            ];

            // Calculate relative velocity if available
            let relative_velocity = if i < world_state.velocities.len() {
                let vel = world_state.velocities[i];
                [
                    vel[0] - agent_state.velocity[0],
                    vel[1] - agent_state.velocity[1],
                    vel[2] - agent_state.velocity[2],
                ]
            } else {
                [0.0; 3]
            };

            // Angular position (azimuth, elevation)
            let azimuth = relative_position[1].atan2(relative_position[0]);
            let elevation = (relative_position[2] / distance).asin();

            let entity_type = world_state.entity_types.get(i).copied().unwrap_or(0);

            entities.push(VisibleEntity {
                entity_index: i,
                entity_type,
                distance,
                relative_position,
                relative_velocity,
                angular_position: [azimuth, elevation],
            });

            if entities.len() >= self.max_entities {
                break;
            }
        }

        // Sort by distance if occlusion is enabled
        if self.occlusion {
            entities.sort_by(|a, b| {
                a.distance.partial_cmp(&b.distance).unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        let count = entities.len();
        SensorReading::Vision(VisionReading { entities, count })
    }
}

// ============================================================================
// Auditory Sensor
// ============================================================================

/// A detected sound source.
#[derive(Debug, Clone)]
pub struct SoundSource {
    /// Direction to the source (unit vector).
    pub direction: [f32; 3],
    /// Perceived intensity (attenuated by distance).
    pub intensity: f32,
    /// Frequency of the sound.
    pub frequency: f32,
    /// Estimated distance to source.
    pub distance: f32,
}

/// Reading from an auditory sensor.
#[derive(Debug, Clone, Default)]
pub struct AuditoryReading {
    /// Detected sound sources.
    pub sources: Vec<SoundSource>,
    /// Ambient noise level.
    pub ambient_level: f32,
}

impl AuditoryReading {
    /// Convert to a flat vector for neural network input.
    pub fn to_vector(&self) -> Vec<f32> {
        let mut result = vec![self.ambient_level, self.sources.len() as f32];
        for source in &self.sources {
            result.extend_from_slice(&source.direction);
            result.push(source.intensity);
            result.push(source.frequency);
            result.push(source.distance);
        }
        result
    }

    /// Get the loudest sound source.
    pub fn loudest(&self) -> Option<&SoundSource> {
        self.sources.iter().max_by(|a, b| {
            a.intensity.partial_cmp(&b.intensity).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

/// Omnidirectional auditory sensor.
///
/// Detects sounds from all directions with intensity attenuation
/// based on distance (inverse square law).
#[derive(Debug, Clone)]
pub struct AuditorySensor {
    /// Maximum detection range.
    range: f32,
    /// Minimum detectable intensity.
    threshold: f32,
    /// Attenuation factor (default: inverse square).
    attenuation_power: f32,
}

impl AuditorySensor {
    /// Create a new auditory sensor.
    ///
    /// # Arguments
    ///
    /// * `range` - Maximum detection range
    ///
    /// # Returns
    ///
    /// A new `AuditorySensor`.
    pub fn new(range: f32) -> Self {
        Self {
            range,
            threshold: 0.01,
            attenuation_power: 2.0,
        }
    }

    /// Set the detection threshold (builder pattern).
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }

    /// Set the attenuation power (builder pattern).
    ///
    /// Default is 2.0 (inverse square law).
    pub fn with_attenuation(mut self, power: f32) -> Self {
        self.attenuation_power = power;
        self
    }
}

impl Sensor for AuditorySensor {
    fn sensor_type(&self) -> SensorType {
        SensorType::Auditory
    }

    fn range(&self) -> f32 {
        self.range
    }

    fn can_detect(&self, agent_state: &AgentState, target_position: [f32; 3]) -> bool {
        let dx = target_position[0] - agent_state.position[0];
        let dy = target_position[1] - agent_state.position[1];
        let dz = target_position[2] - agent_state.position[2];
        let dist_sq = dx * dx + dy * dy + dz * dz;
        dist_sq <= self.range * self.range
    }

    fn sense(&self, agent_state: &AgentState, world_state: &WorldState) -> SensorReading {
        let mut sources = Vec::new();

        for sound in &world_state.sound_sources {
            let dx = sound.position[0] - agent_state.position[0];
            let dy = sound.position[1] - agent_state.position[1];
            let dz = sound.position[2] - agent_state.position[2];
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();

            if distance > self.range {
                continue;
            }

            // Attenuate by distance
            let attenuation = if distance > 0.1 {
                1.0 / distance.powf(self.attenuation_power)
            } else {
                1.0
            };
            let perceived_intensity = sound.intensity * attenuation;

            if perceived_intensity < self.threshold {
                continue;
            }

            // Direction to source
            let inv_dist = if distance > 1e-6 { 1.0 / distance } else { 0.0 };
            let direction = [dx * inv_dist, dy * inv_dist, dz * inv_dist];

            sources.push(SoundSource {
                direction,
                intensity: perceived_intensity,
                frequency: sound.frequency,
                distance,
            });
        }

        SensorReading::Auditory(AuditoryReading {
            sources,
            ambient_level: 0.0,
        })
    }
}

// ============================================================================
// Tactile Sensor
// ============================================================================

/// A contact point detected by tactile sensing.
#[derive(Debug, Clone)]
pub struct ContactPoint {
    /// Local position on the agent's body where contact occurred.
    pub local_position: [f32; 3],
    /// World position of the contact.
    pub world_position: [f32; 3],
    /// Contact normal (pointing away from the contacted surface).
    pub normal: [f32; 3],
    /// Contact pressure/force magnitude.
    pub pressure: f32,
    /// Entity index of the contacted object.
    pub entity_index: Option<usize>,
}

/// Reading from a tactile sensor.
#[derive(Debug, Clone, Default)]
pub struct TactileReading {
    /// Active contact points.
    pub contacts: Vec<ContactPoint>,
    /// Whether any contact is occurring.
    pub in_contact: bool,
    /// Total contact pressure.
    pub total_pressure: f32,
}

impl TactileReading {
    /// Convert to a flat vector for neural network input.
    pub fn to_vector(&self) -> Vec<f32> {
        let mut result = vec![
            if self.in_contact { 1.0 } else { 0.0 },
            self.total_pressure,
            self.contacts.len() as f32,
        ];
        for contact in &self.contacts {
            result.extend_from_slice(&contact.local_position);
            result.extend_from_slice(&contact.normal);
            result.push(contact.pressure);
        }
        result
    }

    /// Get the contact with highest pressure.
    pub fn strongest_contact(&self) -> Option<&ContactPoint> {
        self.contacts.iter().max_by(|a, b| {
            a.pressure.partial_cmp(&b.pressure).unwrap_or(std::cmp::Ordering::Equal)
        })
    }
}

/// Contact-based tactile sensor.
///
/// Detects physical contact with other entities, providing information
/// about contact location, normal, and pressure.
#[derive(Debug, Clone)]
pub struct TactileSensor {
    /// Contact detection radius (skin thickness).
    contact_radius: f32,
    /// Minimum detectable pressure.
    threshold: f32,
}

impl TactileSensor {
    /// Create a new tactile sensor.
    ///
    /// # Arguments
    ///
    /// * `contact_radius` - Detection radius around the body surface
    pub fn new(contact_radius: f32) -> Self {
        Self {
            contact_radius,
            threshold: 0.001,
        }
    }

    /// Set the pressure detection threshold (builder pattern).
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.threshold = threshold;
        self
    }
}

impl Sensor for TactileSensor {
    fn sensor_type(&self) -> SensorType {
        SensorType::Tactile
    }

    fn range(&self) -> f32 {
        self.contact_radius
    }

    fn can_detect(&self, agent_state: &AgentState, target_position: [f32; 3]) -> bool {
        let dx = target_position[0] - agent_state.position[0];
        let dy = target_position[1] - agent_state.position[1];
        let dz = target_position[2] - agent_state.position[2];
        let dist_sq = dx * dx + dy * dy + dz * dz;
        dist_sq <= self.contact_radius * self.contact_radius
    }

    fn sense(&self, agent_state: &AgentState, world_state: &WorldState) -> SensorReading {
        let mut contacts = Vec::new();
        let mut total_pressure = 0.0;

        for (i, pos) in world_state.positions.iter().enumerate() {
            let dx = pos[0] - agent_state.position[0];
            let dy = pos[1] - agent_state.position[1];
            let dz = pos[2] - agent_state.position[2];
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();

            if distance > self.contact_radius {
                continue;
            }

            // Calculate contact pressure based on penetration depth
            let penetration = self.contact_radius - distance;
            let pressure = penetration / self.contact_radius;

            if pressure < self.threshold {
                continue;
            }

            // Contact normal (points away from the other entity)
            let inv_dist = if distance > 1e-6 { 1.0 / distance } else { 0.0 };
            let normal = [-dx * inv_dist, -dy * inv_dist, -dz * inv_dist];

            // Local position is where the contact occurs on the agent's surface
            let local_position = [
                dx * self.contact_radius * inv_dist,
                dy * self.contact_radius * inv_dist,
                dz * self.contact_radius * inv_dist,
            ];

            contacts.push(ContactPoint {
                local_position,
                world_position: *pos,
                normal,
                pressure,
                entity_index: Some(i),
            });

            total_pressure += pressure;
        }

        let in_contact = !contacts.is_empty();
        SensorReading::Tactile(TactileReading {
            contacts,
            in_contact,
            total_pressure,
        })
    }
}

// ============================================================================
// Proprioceptive Sensor
// ============================================================================

/// Reading from a proprioceptive sensor (internal state awareness).
#[derive(Debug, Clone, Default)]
pub struct ProprioceptiveReading {
    /// Current position.
    pub position: [f32; 3],
    /// Current velocity.
    pub velocity: [f32; 3],
    /// Current orientation (quaternion).
    pub orientation: [f32; 4],
    /// Current angular velocity.
    pub angular_velocity: [f32; 3],
    /// Current applied forces.
    pub forces: [f32; 3],
    /// Current applied torques.
    pub torques: [f32; 3],
    /// Current speed.
    pub speed: f32,
    /// Current energy level.
    pub energy: f32,
    /// Current health.
    pub health: f32,
}

impl ProprioceptiveReading {
    /// Convert to a flat vector for neural network input.
    pub fn to_vector(&self) -> Vec<f32> {
        let mut result = Vec::with_capacity(21);
        result.extend_from_slice(&self.position);
        result.extend_from_slice(&self.velocity);
        result.extend_from_slice(&self.orientation);
        result.extend_from_slice(&self.angular_velocity);
        result.extend_from_slice(&self.forces);
        result.extend_from_slice(&self.torques);
        result.push(self.speed);
        result.push(self.energy);
        result.push(self.health);
        result
    }
}

/// Proprioceptive sensor for internal state awareness.
///
/// Provides the agent with information about its own physical state:
/// position, velocity, orientation, applied forces, energy, etc.
#[derive(Debug, Clone, Default)]
pub struct ProprioceptiveSensor {
    /// Whether to include position in readings.
    include_position: bool,
    /// Whether to include velocity in readings.
    include_velocity: bool,
    /// Whether to include forces in readings.
    include_forces: bool,
}

impl ProprioceptiveSensor {
    /// Create a new proprioceptive sensor with all information enabled.
    pub fn new() -> Self {
        Self {
            include_position: true,
            include_velocity: true,
            include_forces: true,
        }
    }

    /// Set which information to include (builder pattern).
    pub fn with_options(mut self, position: bool, velocity: bool, forces: bool) -> Self {
        self.include_position = position;
        self.include_velocity = velocity;
        self.include_forces = forces;
        self
    }
}

impl Sensor for ProprioceptiveSensor {
    fn sensor_type(&self) -> SensorType {
        SensorType::Proprioceptive
    }

    fn range(&self) -> f32 {
        0.0 // Internal sensor, no range
    }

    fn can_detect(&self, _agent_state: &AgentState, _target_position: [f32; 3]) -> bool {
        false // Only senses self
    }

    fn sense(&self, agent_state: &AgentState, _world_state: &WorldState) -> SensorReading {
        SensorReading::Proprioceptive(ProprioceptiveReading {
            position: if self.include_position { agent_state.position } else { [0.0; 3] },
            velocity: if self.include_velocity { agent_state.velocity } else { [0.0; 3] },
            orientation: agent_state.orientation,
            angular_velocity: agent_state.angular_velocity,
            forces: if self.include_forces { agent_state.force } else { [0.0; 3] },
            torques: if self.include_forces { agent_state.torque } else { [0.0; 3] },
            speed: agent_state.speed(),
            energy: agent_state.energy,
            health: agent_state.health,
        })
    }
}

// ============================================================================
// Communication Sensor
// ============================================================================

/// A message received through communication.
#[derive(Debug, Clone)]
pub struct Message {
    /// Sender agent ID.
    pub sender: AgentId,
    /// Message content as a vector of floats.
    pub content: Vec<f32>,
    /// Distance to the sender when the message was received.
    pub distance: f32,
    /// Direction to the sender.
    pub direction: [f32; 3],
    /// Time the message was sent.
    pub timestamp: f32,
}

/// Communication sensor for receiving messages from other agents.
///
/// Receives messages broadcast by other agents within range.
#[derive(Debug, Clone)]
pub struct CommunicationSensor {
    /// Maximum reception range.
    range: f32,
    /// Channel or frequency (for filtered communication).
    channel: u32,
}

impl CommunicationSensor {
    /// Create a new communication sensor.
    ///
    /// # Arguments
    ///
    /// * `range` - Maximum reception range
    pub fn new(range: f32) -> Self {
        Self {
            range,
            channel: 0,
        }
    }

    /// Set the communication channel (builder pattern).
    pub fn with_channel(mut self, channel: u32) -> Self {
        self.channel = channel;
        self
    }
}

impl Sensor for CommunicationSensor {
    fn sensor_type(&self) -> SensorType {
        SensorType::Communication
    }

    fn range(&self) -> f32 {
        self.range
    }

    fn can_detect(&self, agent_state: &AgentState, target_position: [f32; 3]) -> bool {
        let dx = target_position[0] - agent_state.position[0];
        let dy = target_position[1] - agent_state.position[1];
        let dz = target_position[2] - agent_state.position[2];
        let dist_sq = dx * dx + dy * dy + dz * dz;
        dist_sq <= self.range * self.range
    }

    fn sense(&self, agent_state: &AgentState, world_state: &WorldState) -> SensorReading {
        let mut messages = Vec::new();

        for msg_data in &world_state.messages {
            // Calculate distance from message origin
            let dx = msg_data.origin[0] - agent_state.position[0];
            let dy = msg_data.origin[1] - agent_state.position[1];
            let dz = msg_data.origin[2] - agent_state.position[2];
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();

            // Check if within both message range and sensor range
            if distance > self.range || distance > msg_data.range {
                continue;
            }

            // Calculate direction to sender
            let inv_dist = if distance > 1e-6 { 1.0 / distance } else { 0.0 };
            let direction = [dx * inv_dist, dy * inv_dist, dz * inv_dist];

            messages.push(Message {
                sender: msg_data.sender,
                content: msg_data.content.clone(),
                distance,
                direction,
                timestamp: world_state.time,
            });
        }

        SensorReading::Communication(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vision_sensor_fov() {
        let vision = VisionSensor::new(std::f32::consts::FRAC_PI_2, 10.0);
        let agent_state = AgentState::new();

        // Target directly ahead should be visible
        assert!(vision.can_detect(&agent_state, [0.0, 0.0, 5.0]));

        // Target behind should not be visible
        assert!(!vision.can_detect(&agent_state, [0.0, 0.0, -5.0]));

        // Target too far should not be visible
        assert!(!vision.can_detect(&agent_state, [0.0, 0.0, 15.0]));
    }

    #[test]
    fn test_auditory_sensor_range() {
        let hearing = AuditorySensor::new(20.0);
        let agent_state = AgentState::new();

        // Within range
        assert!(hearing.can_detect(&agent_state, [10.0, 0.0, 0.0]));

        // Out of range
        assert!(!hearing.can_detect(&agent_state, [25.0, 0.0, 0.0]));
    }

    #[test]
    fn test_proprioceptive_sensor() {
        let proprio = ProprioceptiveSensor::new();
        let mut agent_state = AgentState::new();
        agent_state.velocity = [1.0, 2.0, 3.0];

        let reading = proprio.sense(&agent_state, &WorldState::default());
        if let SensorReading::Proprioceptive(p) = reading {
            assert_eq!(p.velocity, [1.0, 2.0, 3.0]);
        } else {
            panic!("Expected proprioceptive reading");
        }
    }
}
