//! Agent definition and implementation

use super::{Policy, Goal, Sensor, Actuator, ProposedAction, ActionKind, SensorReading};
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for agents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(pub u64);

impl AgentId {
    /// Generate a new unique agent ID
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new()
    }
}

/// Current state of an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentState {
    /// Position of the agent body
    pub position: [f32; 3],
    /// Velocity of the agent body
    pub velocity: [f32; 3],
    /// Orientation (quaternion)
    pub orientation: [f32; 4],
    /// Energy level
    pub energy: f32,
    /// Health/integrity
    pub health: f32,
    /// Custom state variables
    pub custom: HashMap<String, f32>,
}

impl Default for AgentState {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            velocity: [0.0; 3],
            orientation: [1.0, 0.0, 0.0, 0.0], // Identity quaternion
            energy: 100.0,
            health: 100.0,
            custom: HashMap::new(),
        }
    }
}

/// Trait for agent implementations
pub trait AgentTrait: Send + Sync {
    /// Get agent ID
    fn id(&self) -> AgentId;

    /// Get agent name
    fn name(&self) -> &str;

    /// Get current state
    fn state(&self) -> &AgentState;

    /// Get mutable state
    fn state_mut(&mut self) -> &mut AgentState;

    /// Get sensors
    fn sensors(&self) -> &[Box<dyn Sensor>];

    /// Get actuators
    fn actuators(&self) -> &[Box<dyn Actuator>];

    /// Get policy
    fn policy(&self) -> &dyn Policy;

    /// Get goals
    fn goals(&self) -> &[Box<dyn Goal>];

    /// Compute action given sensor readings
    fn act(&mut self, readings: &[SensorReading]) -> Result<Vec<ProposedAction>>;

    /// Update internal state
    fn update(&mut self, dt: f32);

    /// Check if agent is alive/active
    fn is_active(&self) -> bool {
        self.state().health > 0.0
    }
}

/// Standard agent implementation
pub struct Agent {
    /// Unique identifier
    id: AgentId,
    /// Agent name
    name: String,
    /// Current state
    state: AgentState,
    /// Sensors for perception
    sensors: Vec<Box<dyn Sensor>>,
    /// Actuators for action
    actuators: Vec<Box<dyn Actuator>>,
    /// Decision-making policy
    policy: Box<dyn Policy>,
    /// Agent goals
    goals: Vec<Box<dyn Goal>>,
    /// Atoms controlled by this agent
    controlled_atoms: Vec<u32>,
}

impl Agent {
    /// Create a new agent builder
    pub fn builder() -> AgentBuilder {
        AgentBuilder::new()
    }

    /// Get controlled atoms
    pub fn controlled_atoms(&self) -> &[u32] {
        &self.controlled_atoms
    }

    /// Add a controlled atom
    pub fn add_controlled_atom(&mut self, atom_id: u32) {
        if !self.controlled_atoms.contains(&atom_id) {
            self.controlled_atoms.push(atom_id);
        }
    }
}

impl AgentTrait for Agent {
    fn id(&self) -> AgentId {
        self.id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn state(&self) -> &AgentState {
        &self.state
    }

    fn state_mut(&mut self) -> &mut AgentState {
        &mut self.state
    }

    fn sensors(&self) -> &[Box<dyn Sensor>] {
        &self.sensors
    }

    fn actuators(&self) -> &[Box<dyn Actuator>] {
        &self.actuators
    }

    fn policy(&self) -> &dyn Policy {
        self.policy.as_ref()
    }

    fn goals(&self) -> &[Box<dyn Goal>] {
        &self.goals
    }

    fn act(&mut self, readings: &[SensorReading]) -> Result<Vec<ProposedAction>> {
        // Evaluate goals
        let goal_values: Vec<f32> = self.goals.iter()
            .map(|g| g.evaluate(&self.state))
            .collect();

        // Get policy output
        let policy_output = self.policy.decide(readings, &goal_values)?;

        // Convert to proposed actions
        let mut actions = Vec::new();
        for (idx, &intensity) in policy_output.action_intensities.iter().enumerate() {
            if intensity.abs() > 0.01 {
                if let Some(actuator) = self.actuators.get(idx) {
                    // Generate action based on actuator type and intensity
                    for &atom_id in &self.controlled_atoms {
                        let action = actuator.generate_action(atom_id, intensity);
                        actions.push(ProposedAction::new(self.id, action)
                            .with_rationale(format!("Policy output: {}", policy_output.rationale)));
                    }
                }
            }
        }

        // Deduct energy for actions
        let total_energy_cost: f32 = actions.iter().map(|a| a.energy_cost).sum();
        self.state.energy -= total_energy_cost;

        Ok(actions)
    }

    fn update(&mut self, dt: f32) {
        // Regenerate some energy over time
        self.state.energy = (self.state.energy + dt * 0.1).min(100.0);
    }
}

/// Builder for creating agents
pub struct AgentBuilder {
    name: String,
    state: AgentState,
    sensors: Vec<Box<dyn Sensor>>,
    actuators: Vec<Box<dyn Actuator>>,
    policy: Option<Box<dyn Policy>>,
    goals: Vec<Box<dyn Goal>>,
    controlled_atoms: Vec<u32>,
}

impl AgentBuilder {
    /// Create a new agent builder
    pub fn new() -> Self {
        Self {
            name: "Agent".to_string(),
            state: AgentState::default(),
            sensors: Vec::new(),
            actuators: Vec::new(),
            policy: None,
            goals: Vec::new(),
            controlled_atoms: Vec::new(),
        }
    }

    /// Set agent name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set initial position
    pub fn with_position(mut self, x: f32, y: f32, z: f32) -> Self {
        self.state.position = [x, y, z];
        self
    }

    /// Add a sensor
    pub fn with_sensor(mut self, sensor: Box<dyn Sensor>) -> Self {
        self.sensors.push(sensor);
        self
    }

    /// Add multiple sensors
    pub fn with_sensors(mut self, sensors: Vec<Box<dyn Sensor>>) -> Self {
        self.sensors.extend(sensors);
        self
    }

    /// Add an actuator
    pub fn with_actuator(mut self, actuator: Box<dyn Actuator>) -> Self {
        self.actuators.push(actuator);
        self
    }

    /// Add multiple actuators
    pub fn with_actuators(mut self, actuators: Vec<Box<dyn Actuator>>) -> Self {
        self.actuators.extend(actuators);
        self
    }

    /// Set policy
    pub fn with_policy(mut self, policy: Box<dyn Policy>) -> Self {
        self.policy = Some(policy);
        self
    }

    /// Add a goal
    pub fn with_goal(mut self, goal: Box<dyn Goal>) -> Self {
        self.goals.push(goal);
        self
    }

    /// Add controlled atoms
    pub fn with_controlled_atoms(mut self, atoms: Vec<u32>) -> Self {
        self.controlled_atoms = atoms;
        self
    }

    /// Build the agent
    pub fn build(self) -> Result<Agent> {
        let policy = self.policy
            .unwrap_or_else(|| Box::new(super::RandomPolicy::new()));

        Ok(Agent {
            id: AgentId::new(),
            name: self.name,
            state: self.state,
            sensors: self.sensors,
            actuators: self.actuators,
            policy,
            goals: self.goals,
            controlled_atoms: self.controlled_atoms,
        })
    }
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_unique() {
        let id1 = AgentId::new();
        let id2 = AgentId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_agent_state_default() {
        let state = AgentState::default();
        assert_eq!(state.energy, 100.0);
        assert_eq!(state.health, 100.0);
    }
}
