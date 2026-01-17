//! # Layer 2: AGENCY
//!
//! The agency layer provides embodied agents that can perceive and act within
//! the physical simulation. Agents have:
//!
//! - **Sensors**: Devices for perceiving the world (distance, force, chemical)
//! - **Actuators**: Devices for affecting the world (force applicators, etc.)
//! - **Policies**: Decision-making functions mapping observations to actions
//! - **Goals**: Objectives that define what the agent is trying to achieve
//!
//! ## Design Philosophy
//!
//! Agents are not omniscient - they can only perceive through their sensors
//! and act through their actuators. This creates an information bottleneck
//! that forces emergent behavior.

pub mod agent;
pub mod policy;
pub mod goal;
pub mod actuator;
pub mod sensor;

pub use agent::{Agent, AgentId, AgentState, AgentTrait, AgentBuilder};
pub use policy::{Policy, PolicyOutput, NeuralPolicy, RuleBasedPolicy, RandomPolicy};
pub use goal::{Goal, GoalEvaluator, GoalStatus};
pub use actuator::{Actuator, ActuatorKind, ForceActuator, VelocityActuator};
pub use sensor::{Sensor, SensorKind, SensorReading, DistanceSensor, ForceSensor, ChemicalSensor};

use crate::error::Result;
use serde::{Deserialize, Serialize};

// ============================================================================
// Action Types
// ============================================================================

/// Unique identifier for actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActionId(pub u64);

impl ActionId {
    /// Generate a new unique action ID
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for ActionId {
    fn default() -> Self {
        Self::new()
    }
}

/// Kind of action that can be performed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActionKind {
    /// Apply a force to an atom
    ApplyForce {
        atom_id: u32,
        force: [f32; 3],
    },
    /// Set velocity of an atom
    SetVelocity {
        atom_id: u32,
        velocity: [f32; 3],
    },
    /// Move an atom by displacement
    MoveAtom {
        atom_id: u32,
        displacement: [f32; 3],
    },
    /// No operation
    Noop,
}

/// A proposed action from an agent (not yet validated)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProposedAction {
    /// Unique action ID
    pub id: ActionId,
    /// Agent proposing this action
    pub agent_id: AgentId,
    /// The action kind
    pub kind: ActionKind,
    /// Priority (higher = more important)
    pub priority: u8,
    /// Timestamp of proposal
    pub timestamp: u64,
    /// Energy cost estimate
    pub energy_cost: f32,
    /// Rationale (for audit)
    pub rationale: String,
}

impl ProposedAction {
    /// Create a new proposed action
    pub fn new(agent_id: AgentId, kind: ActionKind) -> Self {
        Self {
            id: ActionId::new(),
            agent_id,
            kind,
            priority: 128,
            timestamp: 0,
            energy_cost: 0.0,
            rationale: String::new(),
        }
    }

    /// Set priority
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Set rationale
    pub fn with_rationale(mut self, rationale: impl Into<String>) -> Self {
        self.rationale = rationale.into();
        self
    }
}

/// A validated action (approved by governance)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedAction {
    /// The original action
    pub id: ActionId,
    /// Agent that proposed this action
    pub agent_id: AgentId,
    /// The action kind
    pub kind: ActionKind,
    /// Validation timestamp
    pub validated_at: u64,
    /// Governance signature/approval
    pub approval_token: String,
}

impl ValidatedAction {
    /// Create from a proposed action with approval
    pub fn from_proposed(action: ProposedAction, approval_token: String) -> Self {
        Self {
            id: action.id,
            agent_id: action.agent_id,
            kind: action.kind,
            validated_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            approval_token,
        }
    }
}

// ============================================================================
// Agent Registry
// ============================================================================

/// Registry of all agents in the simulation
pub struct AgentRegistry {
    /// All registered agents
    agents: Vec<Box<dyn AgentTrait>>,
    /// Agent ID to index mapping
    index_map: std::collections::HashMap<AgentId, usize>,
}

impl AgentRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            agents: Vec::new(),
            index_map: std::collections::HashMap::new(),
        }
    }

    /// Register a new agent
    pub fn register(&mut self, agent: Box<dyn AgentTrait>) {
        let id = agent.id();
        let idx = self.agents.len();
        self.agents.push(agent);
        self.index_map.insert(id, idx);
    }

    /// Get agent by ID
    pub fn get(&self, id: AgentId) -> Option<&dyn AgentTrait> {
        self.index_map.get(&id)
            .map(|&idx| self.agents[idx].as_ref())
    }

    /// Get mutable agent by ID
    pub fn get_mut(&mut self, id: AgentId) -> Option<&mut (dyn AgentTrait + '_)> {
        let idx = *self.index_map.get(&id)?;
        Some(self.agents[idx].as_mut())
    }

    /// Get all agent IDs
    pub fn ids(&self) -> Vec<AgentId> {
        self.index_map.keys().copied().collect()
    }

    /// Iterate over all agents
    pub fn iter(&self) -> impl Iterator<Item = &dyn AgentTrait> {
        self.agents.iter().map(|a| a.as_ref())
    }

    /// Number of registered agents
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}
