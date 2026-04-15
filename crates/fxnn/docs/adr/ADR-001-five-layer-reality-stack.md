# ADR-001: Five-Layer Reality Stack Architecture

## Status

**Accepted**

## Date

2026-01-11

## Context

FXNN is a high-performance molecular dynamics simulation library. To support intelligent agents operating within physical simulations, we need an architecture that:

1. **Grounds agents in physics**: All agent actions must be physically realizable
2. **Enforces information bottlenecks**: Agents cannot observe the full world state
3. **Supports learning**: Agents should improve through experience
4. **Ensures safety**: Agent actions must be validated and auditable
5. **Enables emergence**: Complex behaviors should emerge from simple rules

Existing agent frameworks typically lack physical grounding, while physics engines lack agent-oriented abstractions. We need a layered architecture that bridges these domains.

## Decision

We will implement a **Five-Layer Reality Stack** architecture:

```
┌─────────────────────────────────────────────────────────────────┐
│                    Layer 5: GOVERNANCE                          │
│  Action gating, permissions, audit logging, budget enforcement  │
├─────────────────────────────────────────────────────────────────┤
│                    Layer 4: MEMORY                              │
│  SONA neural substrate, ReasoningBank, trajectories, EWC++      │
├─────────────────────────────────────────────────────────────────┤
│                    Layer 3: PERCEPTION                          │
│  Partial observability, attention, bandwidth limits, noise      │
├─────────────────────────────────────────────────────────────────┤
│                    Layer 2: AGENCY                              │
│  Agents with sensors, actuators, policies, and goals            │
├─────────────────────────────────────────────────────────────────┤
│                    Layer 1: PHYSICS                             │
│  FXNN core with conservation law validation                     │
└─────────────────────────────────────────────────────────────────┘
```

### Layer 1: PHYSICS

The physics layer provides the ground truth simulation with conservation law validation.

**Key Components:**
- `PhysicsEngine<F, I>`: Wraps FXNN `Simulation` with validation
- `ConservationValidator`: Validates energy, momentum, angular momentum
- `WorldState`: Read-only view for perception layer
- `PhysicalConstraint`: SHAKE and other constraint solvers

**Key Traits:**
```rust
pub trait PhysicsLayer: Send + Sync {
    fn advance(&mut self) -> Result<PhysicsResult>;
    fn apply_actions(&mut self, actions: &[ValidatedAction]) -> Result<()>;
    fn world_state(&self) -> &WorldState;
    fn validate_conservation(&self) -> Result<ConservationReport>;
}
```

**Conservation Laws Enforced:**
- Energy (NVE ensemble): <1% drift (with Coulomb: <12%)
- Linear Momentum: <1e-4 absolute magnitude (f32 precision)
- Angular Momentum: <1e-6 drift
- Mass: Exact conservation
- Charge: Exact conservation

**Validated via 529 automated tests** including:
- `test_momentum_conservation_invariant` (10,000 steps)
- `test_bounded_energy_invariant` (10,000 steps)
- `test_force_clipping_invariant` (1,000 steps)
- `test_no_overlap_invariant` (10,000 steps)

### Layer 2: AGENCY

The agency layer provides embodied agents with sensors, actuators, policies, and goals.

**Key Components:**
- `Agent`: Embodied entity with sensors/actuators
- `Sensor`: Distance, force, chemical, velocity sensors
- `Actuator`: Force, velocity, displacement actuators
- `Policy`: Decision-making (Random, RuleBased, Neural)
- `Goal`: Objectives with reward signals

**Key Traits:**
```rust
pub trait AgentTrait: Send + Sync {
    fn id(&self) -> AgentId;
    fn sensors(&self) -> &[Box<dyn Sensor>];
    fn actuators(&self) -> &[Box<dyn Actuator>];
    fn policy(&self) -> &dyn Policy;
    fn goals(&self) -> &[Box<dyn Goal>];
    fn act(&mut self, readings: &[SensorReading]) -> Result<Vec<ProposedAction>>;
}

pub trait Policy: Send + Sync {
    fn decide(&self, readings: &[SensorReading], goals: &[f32]) -> Result<PolicyOutput>;
    fn update(&mut self, reward: f32);
}
```

**Sensor Types:**
| Sensor | Bandwidth | Description |
|--------|-----------|-------------|
| Distance | 64 floats | Nearby atom positions |
| Force | 4 floats | Aggregate force vector |
| Chemical | N floats | Concentration by type |
| Velocity | 4 floats | Local velocity field |

### Layer 3: PERCEPTION

The perception layer provides the information bottleneck between physics and agents.

**Key Components:**
- `Observer`: Generates observations for agents
- `AttentionMask`: Spatial, type-based, velocity-based attention
- `BandwidthLimit`: Constrains observation size
- `NoiseModel`: Gaussian, uniform, distance-dependent noise

**Key Traits:**
```rust
pub trait PerceptionLayer: Send + Sync {
    fn observe(&self, physics: &impl PhysicsLayer) -> Result<Vec<Observation>>;
    fn bandwidth(&self) -> usize;
    fn noise_model(&self) -> &NoiseModel;
}
```

**Bandwidth Constraints:**
- Default: 1024 floats per observation
- Maximum sensors: 16 per agent
- Overflow strategies: Truncate, Sample, AttentionBased, Quantize

### Layer 4: MEMORY

The memory layer provides persistent storage and learning capabilities.

**Key Components:**
- `SONASubstrate`: Self-Optimizing Neural Architecture (<0.05ms adaptation)
- `ReasoningBank`: Experience storage with trajectories
- `EWCProtection`: Elastic Weight Consolidation++ (prevents forgetting)
- `Trajectory`: Observation-action-reward sequences

**Key Traits:**
```rust
pub trait MemoryLayer: Send + Sync {
    fn update(&mut self, observations: &[Observation]) -> Result<()>;
    fn retrieve(&self, agent_id: AgentId, query: &MemoryQuery) -> Vec<MemoryEntry>;
    fn reasoning_bank(&self) -> &ReasoningBank;
    fn store_trajectory(&mut self, trajectory: Trajectory) -> Result<()>;
}
```

**EWC++ Protection:**
- Threshold: 0.95 (high importance memories protected)
- Fisher information tracking
- Online mode with exponential decay
- Prevents catastrophic forgetting

### Layer 5: GOVERNANCE

The governance layer provides safety and control mechanisms.

**Key Components:**
- `ActionGate`: Validates and gates actions
- `PermissionSet`: Role-based permissions
- `Budget`: Resource limits (energy, actions, force)
- `AuditLog`: Persistent action logging

**Key Traits:**
```rust
pub trait GovernanceLayer: Send + Sync {
    fn validate(&mut self, actions: Vec<ProposedAction>) -> Result<Vec<ValidatedAction>>;
    fn is_permitted(&self, action: &ProposedAction) -> bool;
    fn budget(&self) -> &Budget;
    fn audit_log(&self) -> &AuditLog;
}
```

**Permission Levels:**
| Level | Name | Allowed Actions |
|-------|------|-----------------|
| 0 | None | Nothing |
| 1 | Observe | Read-only |
| 2 | Act | Low-impact |
| 3 | Modify | Moderate-impact |
| 4 | Control | High-impact |
| 5 | Admin | Full access |

## Module Structure

```
/crates/fxnn/src/reality_stack/
├── mod.rs                 # Main module with RealityStack struct
├── physics.rs             # Layer 1: Physics engine wrapper
├── agency/
│   ├── mod.rs            # Layer 2: Agency module
│   ├── agent.rs          # Agent implementation
│   ├── sensor.rs         # Sensor implementations
│   ├── actuator.rs       # Actuator implementations
│   ├── policy.rs         # Policy implementations
│   └── goal.rs           # Goal implementations
├── perception.rs         # Layer 3: Perception system
├── memory.rs             # Layer 4: Memory/learning
├── governance.rs         # Layer 5: Action gating
├── witness.rs            # Logging and snapshots
└── benchmark/
    ├── mod.rs            # Benchmark suite
    ├── physics_closure.rs # Conservation benchmarks
    ├── agency.rs         # Agent benchmarks
    └── emergence.rs      # Emergence detection
```

## Alternatives Considered

### Alternative 1: Flat Architecture

**Approach:** Single module with all functionality.

**Rejected because:**
- No clear separation of concerns
- Difficult to enforce invariants between layers
- Hard to test individual components

### Alternative 2: Actor Model

**Approach:** Each agent as an independent actor with message passing.

**Rejected because:**
- Overhead for high-frequency physics updates
- Complex synchronization with physics engine
- Harder to maintain global invariants

### Alternative 3: ECS (Entity Component System)

**Approach:** Components for sensors, actuators, etc.

**Partially adopted:**
- Good for composition (used in Agent)
- Not used for cross-cutting concerns (governance, memory)

## Consequences

### Positive

1. **Clear boundaries**: Each layer has well-defined responsibilities
2. **Physical grounding**: All actions validated against physics
3. **Testability**: Each layer can be tested independently
4. **Safety**: Governance layer prevents dangerous actions
5. **Emergence**: Information bottleneck enables emergent behavior
6. **Learning**: Memory layer supports continual learning with EWC++

### Negative

1. **Overhead**: Layer transitions add latency (~10-50us per step)
2. **Complexity**: Five layers require understanding of all components
3. **Coupling**: Memory layer tightly coupled to perception format

### Mitigation

1. **Overhead**: Use trait objects only at layer boundaries, inline hot paths
2. **Complexity**: Extensive documentation and examples
3. **Coupling**: Define stable observation format, version the protocol

## Performance Targets

| Metric | Target | Measured |
|--------|--------|----------|
| Physics step (1K atoms) | <100us | ~1.3ms |
| Physics step (10K atoms) | <20ms | ~17ms |
| SIMD distance (10K atoms) | <100us | ~90us |
| Perception | <50us per agent | - |
| Policy decision | <100us | - |
| Memory update | <10us | - |
| Governance validation | <5us per action | - |
| Total step (10 agents) | <1ms | - |

## WASM/MCP Integration

### Overview

FXNN includes a lightweight Model Context Protocol (MCP) implementation for WebAssembly, enabling AI agents to interact with molecular dynamics simulations through a standardized JSON-RPC 2.0 interface.

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                      AI Agent (Claude, etc.)                     │
└──────────────────────────────┬──────────────────────────────────┘
                               │ JSON-RPC 2.0
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                      MCP Handler (WASM)                          │
│  - Request parsing and validation                                │
│  - Tool routing and dispatch                                     │
│  - Response serialization                                        │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Simulation Manager                            │
│  - Multiple simulation instances                                 │
│  - Lifecycle management (create/step/destroy)                    │
│  - State queries and configuration                               │
└──────────────────────────────┬──────────────────────────────────┘
                               │
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                       FXNN Core                                  │
│  - Lennard-Jones force fields                                    │
│  - Velocity Verlet integrator                                    │
│  - Conservation law validation                                   │
└─────────────────────────────────────────────────────────────────┘
```

### MCP Tools

| Tool | Description |
|------|-------------|
| `simulation.create` | Create new simulation with atom count, box size, temperature |
| `simulation.step` | Advance simulation by N steps |
| `simulation.state` | Get current atom positions and velocities |
| `simulation.energy` | Get kinetic, potential, and total energy |
| `simulation.configure` | Update timestep and temperature |
| `simulation.destroy` | Clean up simulation instance |
| `simulation.list` | List all active simulation instances |

### MCP Resources

| Resource URI | Description |
|--------------|-------------|
| `fxnn://config/defaults` | Default simulation parameters (timestep, box size, etc.) |
| `fxnn://config/forcefields` | Available force field types and parameters |
| `fxnn://docs/api` | Complete API documentation in Markdown |
| `fxnn://simulation/{id}/positions` | Current atom positions for a simulation |
| `fxnn://simulation/{id}/velocities` | Current atom velocities for a simulation |
| `fxnn://simulation/{id}/state` | Complete simulation state snapshot |

### Request/Response Format

```json
// Request
{
    "jsonrpc": "2.0",
    "id": 1,
    "method": "tools/call",
    "params": {
        "name": "simulation.create",
        "arguments": {
            "n_atoms": 1000,
            "box_size": 50.0,
            "temperature": 300.0
        }
    }
}

// Response
{
    "jsonrpc": "2.0",
    "id": 1,
    "result": {
        "content": [
            {
                "type": "text",
                "text": "{\"simulation_id\":\"sim_abc123\",\"n_atoms\":1000}"
            }
        ]
    }
}
```

### WASM Feature Flag

Enable WASM/MCP support with the `wasm` feature:

```toml
[dependencies]
fxnn = { version = "0.1", features = ["wasm"] }
```

## Security Validations

### Input Validation (Division-by-Zero Prevention)

The following validations prevent undefined behavior and panics:

| Component | Validation | Error |
|-----------|------------|-------|
| `Atom::new` | `mass > 0` | Panics if mass ≤ 0 |
| `SimulationBox::cubic` | `length > 1e-10` | Panics if too small |
| `SimulationBox::orthorhombic` | All dimensions > 1e-10 | Panics if any too small |
| `PairParameters::new` | `sigma > 1e-10` | Panics if sigma too small |
| `PairParameters::new` | `cutoff > 1e-10` | Panics if cutoff too small |
| `PairParameters::new` | `epsilon >= 0` | Panics if negative |

### Unchecked Constructors

For performance-critical code paths where validation has already occurred upstream, unchecked variants are provided:

- `Atom::new_unchecked()` - Bypasses mass validation
- Document `#[doc(hidden)]` to discourage casual use

### Force Clamping (ADR-001 Invariant)

Forces are symmetrically clamped to preserve Newton's Third Law:

```rust
const MAX_FORCE: f32 = 1e6;

// Symmetric clamping preserves f_ij = -f_ji
let scale = if f_mag > MAX_FORCE { MAX_FORCE / f_mag } else { 1.0 };
f_ij = [f_ij[0] * scale, f_ij[1] * scale, f_ij[2] * scale];
```

### Overlap Handling

When atoms overlap (r < sigma/2), a soft repulsion is applied instead of divergent forces:

```rust
const OVERLAP_THRESHOLD: f32 = 0.5; // sigma/2
if r < sigma * OVERLAP_THRESHOLD {
    let soft_r = sigma * OVERLAP_THRESHOLD;
    // Calculate force at soft_r instead of r
}
```

## Benchmarks

Three benchmark categories validate the architecture:

1. **Physics Closure**: Verify conservation laws hold
   - Energy drift < 1% over 10,000 steps (NVE)
   - Momentum drift < 1e-4 over 10,000 steps (f32)

2. **Agency**: Measure decision-making performance
   - >10,000 decisions/second per agent
   - <100us latency per decision

3. **Emergence**: Detect emergent behaviors
   - Spatial entropy measurement
   - Clustering coefficient
   - Order parameter
   - Complexity metrics

### Benchmark Results (2026-01-11)

| Benchmark | Atoms | Time | Notes |
|-----------|-------|------|-------|
| lennard_jones_100 | 100 | 15.5µs | Force calculation only |
| lennard_jones_1000 | 1,000 | 360µs | Force calculation only |
| lennard_jones_10000 | 10,000 | 46ms | Force calculation only |
| full_step_1000 | 1,000 | 1.3ms | Complete simulation step |
| full_step_10000 | 10,000 | 17ms | Complete simulation step |
| simd_distance_10000 | 10,000 | 90µs | SIMD distance calculations |
| neighbor_list_build | 10,000 | 2.1ms | Cell list construction |

Run benchmarks with:
```bash
cd crates/fxnn && cargo bench
```

## Related Decisions

- ADR-002: SONA Neural Substrate Design
- ADR-003: Sensor Specification Protocol
- ADR-004: Governance Policy Language

## References

- Frenkel & Smit, "Understanding Molecular Simulation"
- Russell & Norvig, "Artificial Intelligence: A Modern Approach"
- Kirkpatrick et al., "Overcoming catastrophic forgetting in neural networks" (EWC)
- Schwarz et al., "Progress & Compress" (EWC++)
- Model Context Protocol Specification, Anthropic (2024)

## Changelog

### 2026-01-11 - v0.1.0-alpha

**Added:**
- WASM/MCP Integration section documenting the Model Context Protocol implementation
- Security Validations section documenting input validation and division-by-zero prevention
- Benchmark Results table with actual measured performance
- Test validation counts (529 automated tests)

**Updated:**
- Conservation Laws thresholds to match actual test assertions
- Performance Targets with measured values
- Benchmarks section with detailed results table

**Security:**
- Input validation for `Atom::new` (mass > 0)
- Input validation for `SimulationBox` (dimensions > 1e-10)
- Input validation for `PairParameters` (sigma, cutoff > 1e-10)
- Symmetric force clamping preserving Newton's Third Law
- Soft repulsion for overlapping atoms
