# Cognitum - Constrained Reality Sandbox

A high-performance simulation engine in Rust that creates deterministic, replayable environments where agents can learn within enforced physical and logical constraints.

## Philosophy

**This is a small, strict world that runs inside a computer and refuses to let impossible things happen.**

Cognitum is not just a physics engine - it's a reality-checking sandbox where:

- **State persists** - Every action has consequences that remain
- **Rules are enforced** - Invalid states are rejected, not ignored
- **Everything is deterministic** - Same inputs always produce same outputs
- **All actions are witnessed** - Tamper-evident audit trail with cryptographic hashes
- **Intelligence must play by the rules** - Agents succeed by understanding constraints, not bypassing them

## Core Capabilities

### Simulation Types

| Type | Description | Use Cases |
|------|-------------|-----------|
| **Molecular Dynamics** | Physics-based particle simulations with force fields | Drug discovery, materials science, protein folding |
| **Agent-Based** | Single agent with observations, policy, and learning | Reinforcement learning, decision making |
| **Multi-Agent** | Swarm coordination and emergent behavior | Social dynamics, market simulation, collective intelligence |
| **Gridworld** | Discrete environments for RL and planning | Pathfinding, puzzle solving, game AI |
| **Physics Sandbox** | General physics experiments with constraints | Engineering simulation, robotics |
| **Control Systems** | Feedback loops, PID controllers, stability analysis | Industrial automation, control theory |

### Key Features

- **Deterministic Execution**: Bit-exact reproducibility with configurable validation modes
- **Witness Logging**: Blake3 hashed state transitions for tamper-evident audit trails
- **Constraint Enforcement**: Invalid actions are rejected with detailed error information
- **Snapshot/Restore**: Save and restore complete simulation state at any point
- **MCP Protocol**: JSON-RPC 2.0 interface for AI agent communication
- **WASM Support**: Run simulations in browsers or sandboxed environments
- **SIMD Optimization**: Vectorized calculations using the `wide` crate
- **Parallel Execution**: Optional Rayon-based parallelization for large systems

## Installation

Add Cognitum to your `Cargo.toml`:

```toml
[dependencies]
fxnn = "0.1"
```

### Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `simd` | Yes | SIMD-optimized force calculations |
| `parallel` | Yes | Multi-threaded execution via Rayon |
| `neural` | No | Neural network force fields (SchNet) |
| `wasm` | No | WebAssembly support with MCP integration |
| `python` | No | Python bindings via PyO3 |

## Quick Start

### Molecular Dynamics Simulation

```rust
use fxnn::{Simulation, SimulationBox, LennardJones, VelocityVerlet};
use fxnn::generators::{fcc_lattice, maxwell_boltzmann_velocities};

fn main() {
    // Create a face-centered cubic lattice
    let mut atoms = fcc_lattice(4, 4, 4, 1.5);
    maxwell_boltzmann_velocities(&mut atoms, 1.0, 1.0);

    // Build simulation with constraints
    let box_ = SimulationBox::cubic(6.0);
    let lj = LennardJones::argon();
    let integrator = VelocityVerlet::new();

    let mut sim = Simulation::new(atoms, box_, lj, integrator)
        .with_timestep(0.001)
        .with_witness_logging(true);

    // Run with full state verification
    for _ in 0..100 {
        sim.run(100);
        println!("Step {}: E={:.4}, hash={}",
            sim.step(),
            sim.total_energy(),
            sim.state_hash()
        );
    }
}
```

### Agent-Based Simulation

```rust
use fxnn::agency::{Agent, Environment, Policy};

fn main() {
    // Create environment with constraints
    let env = Environment::gridworld(10, 10)
        .with_obstacles(vec![(5, 5), (5, 6), (6, 5)])
        .with_goal((9, 9));

    // Create agent with random policy
    let mut agent = Agent::new()
        .with_policy(Policy::random())
        .with_learning_rate(0.1);

    // Run episodes with witness logging
    for episode in 0..1000 {
        let mut state = env.reset();
        let mut total_reward = 0.0;

        while !state.is_terminal() {
            let action = agent.act(&state);
            let (next_state, reward, done) = env.step(action);
            agent.learn(&state, action, reward, &next_state);
            total_reward += reward;
            state = next_state;
        }

        println!("Episode {}: reward={:.2}", episode, total_reward);
    }
}
```

## Architecture

### Reality Stack (ADR-001)

```
┌─────────────────────────────────────────────────┐
│             Layer 5: Governance                  │
│  Rules about rules, meta-constraints, policies   │
├─────────────────────────────────────────────────┤
│             Layer 4: Agency                      │
│  Agents, goals, learning, decision-making        │
├─────────────────────────────────────────────────┤
│             Layer 3: Perception                  │
│  Observations, sensors, attention, memory        │
├─────────────────────────────────────────────────┤
│             Layer 2: Witness                     │
│  Hash chains, state verification, audit trails   │
├─────────────────────────────────────────────────┤
│             Layer 1: Physics                     │
│  MD engine, force fields, integrators, SIMD      │
└─────────────────────────────────────────────────┘
```

### Module Organization

```
fxnn/
├── src/
│   ├── types/          # Core data structures (Atom, SimulationBox)
│   ├── force_field/    # Force field implementations
│   │   ├── lennard_jones.rs
│   │   ├── coulomb.rs
│   │   ├── bonded.rs
│   │   └── neural/     # ML-based force fields
│   ├── integrator/     # Time integration schemes
│   │   ├── velocity_verlet.rs  # NVE
│   │   └── langevin.rs         # NVT
│   ├── neighbor/       # Neighbor list algorithms
│   ├── witness/        # Hash chains and verification
│   ├── agency/         # Agent-based simulation
│   ├── perception/     # Observation and sensing
│   ├── governance/     # Rules and constraints
│   ├── wasm/           # WebAssembly bindings
│   └── benchmark/      # Performance tests
└── tests/              # Integration tests
```

## MCP Protocol (cognitum.* Tools)

Cognitum exposes its functionality via Model Context Protocol for AI agent integration:

### Tools

| Tool | Description |
|------|-------------|
| `cognitum.create` | Create new simulation with configuration |
| `cognitum.load_scenario` | Load predefined scenario |
| `cognitum.step` | Advance simulation by N steps |
| `cognitum.snapshot` | Save current state |
| `cognitum.restore` | Restore from snapshot |
| `cognitum.observe` | Get observation from perspective |
| `cognitum.metrics.get` | Get performance metrics |
| `cognitum.memory.put_episode` | Store episode in memory |
| `cognitum.memory.search` | Search episodic memory |
| `cognitum.bench.run` | Run performance benchmark |

### Resources

| Resource | Description |
|----------|-------------|
| `cognitum://config/defaults` | Default parameters |
| `cognitum://config/scenarios` | Available scenarios |
| `cognitum://docs/api` | API documentation |
| `cognitum://simulation/{id}/state` | Simulation snapshot |

### Browser Usage (WASM)

```javascript
import init, { McpHandler, WasmSimulation } from 'fxnn';

async function main() {
    await init();

    // Create MCP handler
    const mcp = new McpHandler();

    // Create simulation via MCP
    const result = mcp.handle_request(JSON.stringify({
        jsonrpc: "2.0", id: 1,
        method: "tools/call",
        params: {
            name: "cognitum.create",
            arguments: {
                type: "molecular",
                lattice_type: "fcc",
                nx: 4, ny: 4, nz: 4,
                determinism: "standard"
            }
        }
    }));

    // Step simulation
    mcp.handle_request(JSON.stringify({
        jsonrpc: "2.0", id: 2,
        method: "tools/call",
        params: {
            name: "cognitum.step",
            arguments: { sim_id: "sim_0", steps: 100 }
        }
    }));
}
```

## Determinism Modes

| Mode | Description | Performance |
|------|-------------|-------------|
| `fast` | No validation, maximum speed | 1.0x |
| `standard` | Hash validation at checkpoints | 0.95x |
| `strict` | Full state verification every step | 0.7x |

## Performance

### Algorithmic Complexity

| Operation | Complexity |
|-----------|------------|
| Force calculation (with neighbor list) | O(N) |
| Neighbor list build (cell list) | O(N) |
| State hash (Blake3) | O(N) |
| Snapshot/restore | O(N) |

### Benchmark Results (Single-threaded)

| System Size | Steps/second | With Witness |
|-------------|--------------|--------------|
| 1,000 atoms | ~50,000 | ~45,000 |
| 10,000 atoms | ~5,000 | ~4,500 |
| 100,000 atoms | ~400 | ~360 |

## Why "Cognitum"?

From Latin *cognitus* (known, understood) + *um* (place/thing):

> A place where knowledge is grounded in physical reality.

Unlike hallucination-prone language models, Cognitum provides:

1. **Grounded truth** - States are computed, not generated
2. **Causal consistency** - Effects follow from causes
3. **Verifiable history** - Every transition is hashed and logged
4. **Constraint satisfaction** - Invalid states cannot exist

This makes it ideal for training agents that must operate in the real world, where "making things up" has consequences.

## Examples

See the `examples/` directory:

- `argon_gas.rs` - Simple LJ simulation
- `water_box.rs` - TIP3P water with bonds
- `agent_maze.rs` - RL agent in gridworld
- `multi_agent.rs` - Swarm coordination

```bash
cargo run --example argon_gas --release
cargo run --example agent_maze --release
```

## Documentation

```bash
cargo doc --open
```

### Guides

- **[Getting Started](docs/tutorials/getting-started.md)** - Your first simulation
- **[Force Fields](docs/tutorials/force-fields.md)** - Understanding force field parameters
- **[Agent Training](docs/tutorials/agent-training.md)** - RL in constrained environments
- **[WASM/MCP Integration](docs/tutorials/wasm-mcp.md)** - Browser and AI agent integration
- **[ADR-001: Reality Stack](docs/adr/ADR-001-five-layer-reality-stack.md)** - Architecture decisions

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

## Citation

```bibtex
@software{cognitum,
  title = {Cognitum: Constrained Reality Sandbox},
  url = {https://github.com/ruvnet/newport},
  version = {0.1.0},
  year = {2024}
}
```
