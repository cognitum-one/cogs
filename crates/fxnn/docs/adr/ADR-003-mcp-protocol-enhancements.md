# ADR-003: MCP Protocol Enhancements for Full Reality Stack

## Status

**Accepted**

## Date

2026-01-12

## Context

The FXNN WASM module currently exposes basic simulation tools via MCP (Model Context Protocol):
- `simulation.create` - Create molecular dynamics simulations
- `simulation.step` - Advance simulation steps
- `simulation.state` - Get current state
- `simulation.energy` - Get energy values
- `simulation.configure` - Update parameters
- `simulation.destroy` - Destroy simulation
- `simulation.list` - List active simulations

However, the Five-Layer Reality Stack (ADR-001) defines capabilities across all five layers that are not yet exposed:

| Layer | Documented Capability | MCP Status |
|-------|----------------------|------------|
| L1: Physics | Force calculations, integration | ✅ Implemented |
| L2: Agency | Agents, sensors, actuators | ❌ Not exposed |
| L3: Perception | Observations, attention | ❌ Not exposed |
| L4: Memory | Episodes, trajectories | ❌ Not exposed |
| L5: Governance | Witness logs, constraints | ❌ Partial |

Additionally, core simulation features are missing:
- **Snapshot/Restore**: Critical for reproducibility and debugging
- **Scenario Library**: Pre-built configurations for common use cases
- **Performance Benchmarks**: Runtime metrics for optimization

## Decision

We will extend the MCP protocol with tools covering all five layers of the Reality Stack.

### New MCP Tools

#### 1. Snapshot/Restore (Layer 1: Physics + Layer 5: Governance)

```
simulation.snapshot
  Input: { sim_id: string }
  Output: {
    snapshot_id: string,
    step: number,
    hash: string,        // Blake3 hash of full state
    n_atoms: number,
    timestamp: string
  }

simulation.restore
  Input: { sim_id: string, snapshot_id: string }
  Output: {
    success: boolean,
    restored_step: number,
    hash: string
  }

simulation.snapshots
  Input: { sim_id: string }
  Output: {
    snapshots: [{
      id: string,
      step: number,
      hash: string,
      timestamp: string
    }]
  }
```

**Implementation:**
- Store full atom positions, velocities, and box dimensions
- Compute Blake3 hash for verification
- Store in memory (WASM) or IndexedDB (browser persistence)
- Limit to 100 snapshots per simulation to prevent memory exhaustion

#### 2. Scenario Library (Layer 1: Physics)

```
simulation.scenarios
  Input: {}
  Output: {
    scenarios: [{
      id: string,
      name: string,
      description: string,
      category: "molecular" | "agent" | "benchmark",
      params: object
    }]
  }

simulation.load_scenario
  Input: { scenario_id: string, overrides?: object }
  Output: {
    sim_id: string,
    loaded_scenario: string,
    n_atoms: number,
    description: string
  }
```

**Built-in Scenarios:**

| ID | Name | Description | Atoms |
|----|------|-------------|-------|
| `argon_256` | Argon Gas (Small) | 256 Ar atoms, LJ potential | 256 |
| `argon_2048` | Argon Gas (Medium) | 2048 Ar atoms, LJ potential | 2048 |
| `argon_16k` | Argon Gas (Large) | 16384 Ar atoms, LJ potential | 16384 |
| `crystal_fcc` | FCC Crystal | Perfect FCC lattice, low temp | 500 |
| `liquid_lj` | LJ Liquid | Lennard-Jones liquid at T=1.0 | 1000 |
| `binary_mixture` | Binary Mixture | Two-component LJ system | 1000 |
| `phase_transition` | Phase Transition | System near melting point | 2048 |

#### 3. Witness/Audit Trail (Layer 5: Governance)

```
simulation.witness
  Input: { sim_id: string, limit?: number }
  Output: {
    entries: [{
      step: number,
      hash: string,
      prev_hash: string,
      event_type: "step" | "snapshot" | "restore" | "configure",
      timestamp: string,
      data?: object
    }]
  }

simulation.verify
  Input: { sim_id: string, from_step?: number, to_step?: number }
  Output: {
    valid: boolean,
    checked_steps: number,
    first_invalid?: number,
    error?: string
  }
```

**Implementation:**
- Every 100 steps (configurable), compute and store state hash
- Chain hashes: `hash_n = Blake3(hash_{n-1} || state_n)`
- Verification recomputes chain and compares
- Enables tamper detection for audit trails

#### 4. Observation System (Layer 2: Agency + Layer 3: Perception)

```
simulation.observe
  Input: {
    sim_id: string,
    observer_type: "global" | "local" | "agent",
    center?: [x, y, z],      // For local observation
    radius?: number,          // Observation radius
    agent_id?: string         // For agent perspective
  }
  Output: {
    observation: number[],    // Flattened observation vector
    shape: number[],          // Shape for reconstruction
    metadata: {
      n_visible: number,
      center: [x, y, z],
      energy_local?: number
    }
  }

simulation.agent_create
  Input: {
    sim_id: string,
    position: [x, y, z],
    observation_radius: number,
    action_type: "force" | "velocity"
  }
  Output: {
    agent_id: string,
    observation: number[]
  }

simulation.agent_act
  Input: {
    sim_id: string,
    agent_id: string,
    action: number[]
  }
  Output: {
    next_observation: number[],
    reward: number,
    done: boolean,
    info: {
      step: number,
      energy: number
    }
  }
```

**Observation Encoding:**
- Local: Relative positions/velocities of nearby atoms within radius
- Global: Statistics (mean positions, temperature, pressure)
- Agent: Egocentric view from agent's perspective

#### 5. Episodic Memory (Layer 4: Memory)

```
simulation.memory_store
  Input: {
    sim_id: string,
    key: string,
    episode: {
      observations: number[][],
      actions: number[][],
      rewards: number[],
      metadata?: object
    }
  }
  Output: { stored: boolean, episode_id: string }

simulation.memory_search
  Input: {
    sim_id: string,
    query: string | number[],  // Text or embedding
    limit?: number,
    threshold?: number
  }
  Output: {
    results: [{
      episode_id: string,
      key: string,
      score: number,
      summary: object
    }]
  }

simulation.memory_replay
  Input: { sim_id: string, episode_id: string }
  Output: {
    observations: number[][],
    actions: number[][],
    rewards: number[],
    total_reward: number
  }
```

**Implementation:**
- Store episodes as serialized binary blobs
- Simple cosine similarity for search (no HNSW in WASM)
- Limit to 1000 episodes per simulation

#### 6. Benchmarks (Layer 1: Physics)

```
simulation.bench
  Input: {
    suite: "all" | "force" | "neighbor" | "integrate" | "hash",
    n_atoms?: number,
    n_steps?: number
  }
  Output: {
    results: [{
      name: string,
      n_atoms: number,
      steps_per_second: number,
      time_per_step_us: number,
      memory_mb: number
    }],
    system_info: {
      wasm: boolean,
      simd: boolean,
      threads: number
    }
  }
```

### Tool Naming Convention

All tools use the `simulation.*` namespace (not `cognitum.*` as originally documented):

```
simulation.{resource}.{action}
  └── simulation.snapshot     (save state)
  └── simulation.restore      (load state)
  └── simulation.observe      (get observation)
  └── simulation.memory_store (save episode)
  └── simulation.bench        (run benchmarks)
```

## Implementation Strategy

### Phase 1: Core Reproducibility (Priority: Critical)
1. `simulation.snapshot` - Save full state with hash
2. `simulation.restore` - Restore from snapshot
3. `simulation.snapshots` - List available snapshots
4. `simulation.witness` - Get audit trail
5. `simulation.verify` - Verify hash chain

### Phase 2: Scenario Library (Priority: High)
1. `simulation.scenarios` - List available scenarios
2. `simulation.load_scenario` - Create simulation from scenario

### Phase 3: Agent/RL Support (Priority: Medium)
1. `simulation.observe` - Get observations
2. `simulation.agent_create` - Create embodied agent
3. `simulation.agent_act` - Execute agent action

### Phase 4: Memory & Benchmarks (Priority: Low)
1. `simulation.memory_store` - Store episodes
2. `simulation.memory_search` - Search episodes
3. `simulation.memory_replay` - Replay episode
4. `simulation.bench` - Run benchmarks

## Data Structures

### Snapshot Storage

```rust
#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub sim_id: String,
    pub step: u64,
    pub hash: String,
    pub prev_hash: Option<String>,
    pub timestamp: String,
    pub positions: Vec<[f32; 3]>,
    pub velocities: Vec<[f32; 3]>,
    pub box_size: [f32; 3],
}
```

### Witness Entry

```rust
#[derive(Serialize, Deserialize)]
pub struct WitnessEntry {
    pub step: u64,
    pub hash: String,
    pub prev_hash: String,
    pub event_type: WitnessEventType,
    pub timestamp: String,
    pub data: Option<Value>,
}

#[derive(Serialize, Deserialize)]
pub enum WitnessEventType {
    Step,
    Snapshot,
    Restore,
    Configure,
    AgentAction,
}
```

### Episode

```rust
#[derive(Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub key: String,
    pub sim_id: String,
    pub observations: Vec<Vec<f32>>,
    pub actions: Vec<Vec<f32>>,
    pub rewards: Vec<f32>,
    pub total_reward: f32,
    pub metadata: Option<Value>,
}
```

## Consequences

### Positive
- Full Reality Stack exposed via MCP
- Reproducible simulations with cryptographic verification
- Quick-start scenarios for common use cases
- Agent training infrastructure built-in
- Performance visibility through benchmarks

### Negative
- Increased WASM binary size (~50KB additional)
- Memory overhead for snapshots/episodes
- Complexity of maintaining hash chains

### Mitigations
- Lazy loading of scenario definitions
- Configurable snapshot limits
- Optional witness logging (can be disabled for performance)

## References

- [ADR-001: Five-Layer Reality Stack](./ADR-001-five-layer-reality-stack.md)
- [ADR-002: Dashboard Integration](./ADR-002-fxnn-dashboard-integration.md)
- [MCP Specification](https://modelcontextprotocol.io)
- [Blake3 Hash Function](https://github.com/BLAKE3-team/BLAKE3)
