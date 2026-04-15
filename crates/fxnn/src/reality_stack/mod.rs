//! # Five-Layer Reality Stack
//!
//! The Reality Stack provides a layered abstraction for building intelligent agents
//! that operate within physical simulations while maintaining safety guarantees.
//!
//! ## Architecture Overview
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Layer 5: GOVERNANCE                          │
//! │  Action gating, permissions, audit logging, budget enforcement  │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                    Layer 4: MEMORY                              │
//! │  SONA neural substrate, ReasoningBank, trajectories, EWC++      │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                    Layer 3: PERCEPTION                          │
//! │  Partial observability, attention, bandwidth limits, noise      │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                    Layer 2: AGENCY                              │
//! │  Agents with sensors, actuators, policies, and goals            │
//! ├─────────────────────────────────────────────────────────────────┤
//! │                    Layer 1: PHYSICS                             │
//! │  FXNN core with conservation law validation                     │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Design Principles
//!
//! 1. **Physical Grounding**: All agent actions must be physically realizable
//! 2. **Information Bottleneck**: Agents cannot observe the full world state
//! 3. **Bounded Rationality**: Memory and compute budgets constrain reasoning
//! 4. **Verifiable Safety**: Governance layer ensures all actions are auditable
//! 5. **Emergent Behavior**: Complex behaviors emerge from simple physical rules
//!
//! ## Layer Interactions
//!
//! ```text
//! ┌──────────────┐     ┌──────────────┐     ┌──────────────┐
//! │   PHYSICS    │────▶│    AGENCY    │────▶│  PERCEPTION  │
//! │  (ground     │     │  (embodied   │     │  (filtered   │
//! │   truth)     │     │   entities)  │     │   views)     │
//! └──────────────┘     └──────────────┘     └──────────────┘
//!        ▲                    │                    │
//!        │                    ▼                    ▼
//!        │             ┌──────────────┐     ┌──────────────┐
//!        └─────────────│  GOVERNANCE  │◀────│    MEMORY    │
//!          (validated  │  (gated      │     │  (learned    │
//!           actions)   │   actions)   │     │   patterns)  │
//!                      └──────────────┘     └──────────────┘
//! ```
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use fxnn::reality_stack::{
//!     physics::{PhysicsEngine, ConservationValidator},
//!     agency::{Agent, Policy, Goal},
//!     perception::{Observer, AttentionMask},
//!     memory::{SONASubstrate, ReasoningBank},
//!     governance::{ActionGate, AuditLog},
//! };
//!
//! // Create physics layer with validation
//! let physics = PhysicsEngine::new()
//!     .with_validator(ConservationValidator::strict());
//!
//! // Create agent with embodiment
//! let agent = Agent::builder()
//!     .with_sensors(vec![DistanceSensor::new(10.0)])
//!     .with_actuators(vec![ForceActuator::new(1.0)])
//!     .with_policy(NeuralPolicy::load("model.bin")?)
//!     .with_goal(Goal::minimize_energy())
//!     .build()?;
//!
//! // Create perception with bandwidth limits
//! let observer = Observer::new()
//!     .with_bandwidth(1024)
//!     .with_noise(0.01)
//!     .with_attention(AttentionMask::spatial(5.0));
//!
//! // Create memory with EWC++ protection
//! let memory = SONASubstrate::new()
//!     .with_reasoning_bank(ReasoningBank::default())
//!     .with_ewc_protection(0.95);
//!
//! // Create governance with action gating
//! let governance = ActionGate::new()
//!     .with_permissions(Permissions::default())
//!     .with_audit(AuditLog::persistent("audit.log"));
//!
//! // Build the full stack
//! let stack = RealityStack::builder()
//!     .physics(physics)
//!     .agent(agent)
//!     .perception(observer)
//!     .memory(memory)
//!     .governance(governance)
//!     .build()?;
//!
//! // Run simulation
//! for _ in 0..1000 {
//!     stack.step()?;
//! }
//! ```

pub mod physics;
pub mod agency;
pub mod perception;
pub mod memory;
pub mod governance;
pub mod witness;

// Re-exports for convenient access
pub use physics::{PhysicsEngine, ConservationValidator, ConservationLaw, PhysicsError};
pub use agency::{Agent, AgentId, Policy, Goal, Sensor, Actuator, AgentState};
pub use perception::{Observer, Observation, AttentionMask, BandwidthLimit, NoiseModel};
pub use memory::{
    SONASubstrate, ReasoningBank, Trajectory, EWCProtection, MemoryError,
    // ADR-001 Learning Safety
    LearningSafetyConfig, LearningSafetyEnforcer, PolicyUpdateValidation, LearningSafetyStats,
};
pub use governance::{ActionGate, Permission, AuditLog, Budget, GovernanceError};
pub use witness::{WitnessLog, Snapshot, Event, EventKind};

use crate::error::{FxnnError, Result};
use crate::types::{Atom, SimulationBox};

/// Unique identifier for reality stack instances
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StackId(pub u64);

impl StackId {
    /// Generate a new unique stack ID
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        Self(COUNTER.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for StackId {
    fn default() -> Self {
        Self::new()
    }
}

/// The Five-Layer Reality Stack
///
/// Integrates all five layers into a coherent system for running
/// intelligent agents within physical simulations.
pub struct RealityStack<P, A, O, M, G>
where
    P: PhysicsLayer,
    A: AgencyLayer,
    O: PerceptionLayer,
    M: MemoryLayer,
    G: GovernanceLayer,
{
    /// Unique identifier for this stack
    pub id: StackId,

    /// Layer 1: Physics engine with conservation validation
    pub physics: P,

    /// Layer 2: Agent system
    pub agency: A,

    /// Layer 3: Perception/observation system
    pub perception: O,

    /// Layer 4: Memory and learning substrate
    pub memory: M,

    /// Layer 5: Governance and action gating
    pub governance: G,

    /// Witness log for auditing
    witness: witness::WitnessLog,

    /// Current simulation step
    step: u64,
}

impl<P, A, O, M, G> RealityStack<P, A, O, M, G>
where
    P: PhysicsLayer,
    A: AgencyLayer,
    O: PerceptionLayer,
    M: MemoryLayer,
    G: GovernanceLayer,
{
    /// Create a new reality stack with all layers
    pub fn new(
        physics: P,
        agency: A,
        perception: O,
        memory: M,
        governance: G,
    ) -> Self {
        Self {
            id: StackId::new(),
            physics,
            agency,
            perception,
            memory,
            governance,
            witness: witness::WitnessLog::new(),
            step: 0,
        }
    }

    /// Execute one simulation step through all layers
    ///
    /// The step proceeds as follows:
    /// 1. Physics: Advance simulation, validate conservation laws
    /// 2. Perception: Generate observations for agents
    /// 3. Memory: Update agent memories with observations
    /// 4. Agency: Compute agent actions from observations + memory
    /// 5. Governance: Validate and gate actions
    /// 6. Physics: Apply validated actions to simulation
    pub fn step(&mut self) -> Result<StepResult> {
        self.witness.begin_step(self.step);

        // 1. Advance physics simulation
        let physics_result = self.physics.advance()?;
        self.witness.record_physics(physics_result.clone());

        // 2. Generate observations for each agent
        let observations = self.perception.observe(&self.physics)?;

        // 3. Update agent memories
        self.memory.update(&observations)?;

        // 4. Compute agent actions
        let proposed_actions = self.agency.act(&observations, &self.memory)?;

        // 5. Validate and gate actions through governance
        let validated_actions = self.governance.validate(proposed_actions)?;
        self.witness.record_actions(&validated_actions);

        // 6. Apply validated actions to physics
        self.physics.apply_actions(&validated_actions)?;

        self.step += 1;
        self.witness.end_step();

        Ok(StepResult {
            step: self.step,
            physics: physics_result,
            n_observations: observations.len(),
            n_actions: validated_actions.len(),
        })
    }

    /// Get current step number
    pub fn current_step(&self) -> u64 {
        self.step
    }

    /// Get reference to witness log
    pub fn witness(&self) -> &witness::WitnessLog {
        &self.witness
    }

    /// Take a snapshot of the current state
    pub fn snapshot(&self) -> witness::Snapshot {
        witness::Snapshot {
            step: self.step,
            timestamp: std::time::Instant::now(),
            stack_id: self.id,
        }
    }
}

/// Result of a single simulation step
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Step number
    pub step: u64,
    /// Physics layer result
    pub physics: physics::PhysicsResult,
    /// Number of observations generated
    pub n_observations: usize,
    /// Number of actions applied
    pub n_actions: usize,
}

// ============================================================================
// Layer Traits
// ============================================================================

/// Layer 1: Physics simulation with conservation validation
pub trait PhysicsLayer: Send + Sync {
    /// Advance the simulation by one timestep
    fn advance(&mut self) -> Result<physics::PhysicsResult>;

    /// Apply validated actions to the simulation
    fn apply_actions(&mut self, actions: &[agency::ValidatedAction]) -> Result<()>;

    /// Get current world state (for perception)
    fn world_state(&self) -> &physics::WorldState;

    /// Validate conservation laws
    fn validate_conservation(&self) -> Result<physics::ConservationReport>;
}

/// Layer 2: Agent system with embodiment
pub trait AgencyLayer: Send + Sync {
    /// Compute actions for all agents given observations and memory
    fn act(
        &mut self,
        observations: &[perception::Observation],
        memory: &impl MemoryLayer,
    ) -> Result<Vec<agency::ProposedAction>>;

    /// Get agent by ID
    fn get_agent(&self, id: AgentId) -> Option<&dyn agency::AgentTrait>;

    /// Get all agent IDs
    fn agent_ids(&self) -> Vec<AgentId>;
}

/// Layer 3: Perception with information bottleneck
pub trait PerceptionLayer: Send + Sync {
    /// Generate observations for all agents
    fn observe(&self, physics: &impl PhysicsLayer) -> Result<Vec<perception::Observation>>;

    /// Get bandwidth limit
    fn bandwidth(&self) -> usize;

    /// Get noise model
    fn noise_model(&self) -> &perception::NoiseModel;
}

/// Layer 4: Memory and learning substrate
pub trait MemoryLayer: Send + Sync {
    /// Update memory with new observations
    fn update(&mut self, observations: &[perception::Observation]) -> Result<()>;

    /// Retrieve relevant memories for an agent
    fn retrieve(&self, agent_id: AgentId, query: &memory::MemoryQuery) -> Vec<memory::MemoryEntry>;

    /// Get reasoning bank
    fn reasoning_bank(&self) -> &memory::ReasoningBank;

    /// Store trajectory
    fn store_trajectory(&mut self, trajectory: memory::Trajectory) -> Result<()>;
}

/// Layer 5: Governance with action gating
pub trait GovernanceLayer: Send + Sync {
    /// Validate proposed actions against policies
    fn validate(
        &mut self,
        actions: Vec<agency::ProposedAction>,
    ) -> Result<Vec<agency::ValidatedAction>>;

    /// Check if an action is permitted
    fn is_permitted(&self, action: &agency::ProposedAction) -> bool;

    /// Get current budget state
    fn budget(&self) -> &governance::Budget;

    /// Get audit log
    fn audit_log(&self) -> &governance::AuditLog;
}
