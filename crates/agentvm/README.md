# Agentic VM

Accountable Agent Execution Runtime with Capability-Based Security

## Overview

Agentic VM provides a secure, auditable runtime environment for AI agents. It implements
capability-based security with full evidence trails, enabling safe and accountable agent
execution.

## Crates

| Crate | Description | no_std |
|-------|-------------|--------|
| `agentvm-types` | Core types (capsules, capabilities, budgets, evidence) | Yes |
| `agentvm-capability` | Capability protocol implementation | Yes |
| `agentvm-evidence` | Evidence chain and Merkle tree | Yes |
| `agentvm-scheduler` | Fabric scheduler for capsule placement | No |
| `agentvm-proxy` | Capability proxy managing resource access | No |
| `agentvm-cli` | Command-line interface | No |

## Quick Start

```bash
# Build all crates
cargo build --workspace

# Run tests
cargo test --workspace

# Run the CLI
cargo run -p agentvm-cli -- --help
```

## Architecture

See [ADR-008](../../adr/ddd/ADR-008-implementation-crates.md) for the detailed crate structure.

```
                              +-------------+
                              | agentvm-cli |
                              +------+------+
                                     |
                  +------------------+------------------+
                  |                  |                  |
                  v                  v                  v
           +-------------+   +-------------+   +-------------+
           | agentvm-vmm |   |agentvm-proxy|   |agentvm-sched|
           +------+------+   +------+------+   +------+------+
                  |                  |                  |
                  +------------------+------------------+
                                     |
                                     v
           +--------------------------------------------+
           |           agentvm-evidence                 |
           +--------------------+-----------------------+
                                |
                                v
           +--------------------------------------------+
           |           agentvm-capability               |
           +--------------------+-----------------------+
                                |
                                v
           +--------------------------------------------+
           |            agentvm-types                   |
           |               (no_std)                     |
           +--------------------------------------------+
```

## Features

- **Capability-Based Security**: Fine-grained access control with derivation
- **Evidence Trails**: Cryptographic proof of all agent actions
- **Budget Control**: Resource limits (CPU, memory, network, etc.)
- **Snapshot/Restore**: Time-travel debugging and recovery
- **Wire Protocol**: Efficient binary protocol for capability invocation

## License

Apache-2.0
