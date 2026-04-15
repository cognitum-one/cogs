//! Actuators for agent actions

use super::ActionKind;
use serde::{Deserialize, Serialize};

/// Unique identifier for actuators
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActuatorId(pub u32);

/// Kind of actuator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActuatorKind {
    /// Applies force to atoms
    Force {
        max_force: f32,
        direction: Option<[f32; 3]>, // None = any direction
    },
    /// Sets velocity of atoms
    Velocity {
        max_velocity: f32,
    },
    /// Moves atoms by displacement
    Displacement {
        max_displacement: f32,
    },
}

/// Trait for actuator implementations
pub trait Actuator: Send + Sync {
    /// Get actuator ID
    fn id(&self) -> ActuatorId;

    /// Get actuator kind
    fn kind(&self) -> ActuatorKind;

    /// Get actuator name
    fn name(&self) -> &str;

    /// Generate an action for a given atom with intensity [-1, 1]
    fn generate_action(&self, atom_id: u32, intensity: f32) -> ActionKind;

    /// Get energy cost per unit intensity
    fn energy_cost_per_intensity(&self) -> f32;

    /// Check if actuator can affect the given atom
    fn can_affect(&self, atom_id: u32) -> bool {
        let _ = atom_id;
        true // Default: can affect any atom
    }
}

/// Force actuator - applies forces to atoms
pub struct ForceActuator {
    id: ActuatorId,
    name: String,
    max_force: f32,
    direction: Option<[f32; 3]>,
    energy_cost: f32,
}

impl ForceActuator {
    /// Create a new force actuator
    pub fn new(max_force: f32) -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);

        Self {
            id: ActuatorId(COUNTER.fetch_add(1, Ordering::SeqCst)),
            name: "ForceActuator".to_string(),
            max_force,
            direction: None,
            energy_cost: 0.01,
        }
    }

    /// Set a fixed direction for force application
    pub fn with_direction(mut self, direction: [f32; 3]) -> Self {
        // Normalize
        let mag = (direction[0].powi(2) + direction[1].powi(2) + direction[2].powi(2)).sqrt();
        if mag > 0.0 {
            self.direction = Some([
                direction[0] / mag,
                direction[1] / mag,
                direction[2] / mag,
            ]);
        }
        self
    }

    /// Set energy cost per unit intensity
    pub fn with_energy_cost(mut self, cost: f32) -> Self {
        self.energy_cost = cost;
        self
    }
}

impl Actuator for ForceActuator {
    fn id(&self) -> ActuatorId {
        self.id
    }

    fn kind(&self) -> ActuatorKind {
        ActuatorKind::Force {
            max_force: self.max_force,
            direction: self.direction,
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn generate_action(&self, atom_id: u32, intensity: f32) -> ActionKind {
        let clamped = intensity.clamp(-1.0, 1.0);
        let force_mag = clamped * self.max_force;

        let force = match self.direction {
            Some(dir) => [
                dir[0] * force_mag,
                dir[1] * force_mag,
                dir[2] * force_mag,
            ],
            None => {
                // Random direction (should be provided by policy in practice)
                [force_mag, 0.0, 0.0]
            }
        };

        ActionKind::ApplyForce { atom_id, force }
    }

    fn energy_cost_per_intensity(&self) -> f32 {
        self.energy_cost
    }
}

/// Velocity actuator - sets velocities of atoms
pub struct VelocityActuator {
    id: ActuatorId,
    name: String,
    max_velocity: f32,
    energy_cost: f32,
}

impl VelocityActuator {
    /// Create a new velocity actuator
    pub fn new(max_velocity: f32) -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);

        Self {
            id: ActuatorId(COUNTER.fetch_add(1, Ordering::SeqCst)),
            name: "VelocityActuator".to_string(),
            max_velocity,
            energy_cost: 0.05,
        }
    }
}

impl Actuator for VelocityActuator {
    fn id(&self) -> ActuatorId {
        self.id
    }

    fn kind(&self) -> ActuatorKind {
        ActuatorKind::Velocity {
            max_velocity: self.max_velocity,
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn generate_action(&self, atom_id: u32, intensity: f32) -> ActionKind {
        let clamped = intensity.clamp(-1.0, 1.0);
        let vel = clamped * self.max_velocity;

        ActionKind::SetVelocity {
            atom_id,
            velocity: [vel, 0.0, 0.0], // Direction should come from policy
        }
    }

    fn energy_cost_per_intensity(&self) -> f32 {
        self.energy_cost
    }
}

/// Displacement actuator - moves atoms by small amounts
pub struct DisplacementActuator {
    id: ActuatorId,
    name: String,
    max_displacement: f32,
    energy_cost: f32,
}

impl DisplacementActuator {
    /// Create a new displacement actuator
    pub fn new(max_displacement: f32) -> Self {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);

        Self {
            id: ActuatorId(COUNTER.fetch_add(1, Ordering::SeqCst)),
            name: "DisplacementActuator".to_string(),
            max_displacement,
            energy_cost: 0.1,
        }
    }
}

impl Actuator for DisplacementActuator {
    fn id(&self) -> ActuatorId {
        self.id
    }

    fn kind(&self) -> ActuatorKind {
        ActuatorKind::Displacement {
            max_displacement: self.max_displacement,
        }
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn generate_action(&self, atom_id: u32, intensity: f32) -> ActionKind {
        let clamped = intensity.clamp(-1.0, 1.0);
        let disp = clamped * self.max_displacement;

        ActionKind::MoveAtom {
            atom_id,
            displacement: [disp, 0.0, 0.0], // Direction should come from policy
        }
    }

    fn energy_cost_per_intensity(&self) -> f32 {
        self.energy_cost
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_force_actuator() {
        let actuator = ForceActuator::new(10.0);
        let action = actuator.generate_action(0, 0.5);

        match action {
            ActionKind::ApplyForce { atom_id, force } => {
                assert_eq!(atom_id, 0);
                assert!((force[0] - 5.0).abs() < 0.01);
            }
            _ => panic!("Expected ApplyForce action"),
        }
    }

    #[test]
    fn test_force_actuator_with_direction() {
        let actuator = ForceActuator::new(10.0)
            .with_direction([0.0, 1.0, 0.0]);
        let action = actuator.generate_action(0, 1.0);

        match action {
            ActionKind::ApplyForce { force, .. } => {
                assert!(force[0].abs() < 0.01);
                assert!((force[1] - 10.0).abs() < 0.01);
                assert!(force[2].abs() < 0.01);
            }
            _ => panic!("Expected ApplyForce action"),
        }
    }
}
