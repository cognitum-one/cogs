# agentvm-scheduler

Fabric scheduler for Agentic VM capsule execution.

## Overview

This crate implements the scheduling architecture for placing and executing
agent capsules across a heterogeneous compute fabric with three tiers:

- **Edge**: Low-power microcontrollers for gating and anomaly detection
- **Host**: Standard compute for agent execution
- **Accel**: GPU/TPU for inference workloads

## Architecture

The scheduler follows a filter-then-score pattern:

1. **Filter Phase**: Eliminate infeasible nodes based on hard constraints
2. **Score Phase**: Rank remaining nodes by soft preferences
3. **Bind Phase**: Commit placement and reserve resources

## Modules

- `task` - Task specification and classification
- `node` - Node registry and capabilities
- `filter` - Filtering plugins (tier, resource, capability)
- `score` - Scoring plugins (affinity, load balance, power)
- `placement` - Placement decisions

## Task Classes

| Class | Preferred Tier | Example |
|-------|---------------|---------|
| `Gating` | Edge | Anomaly detection threshold |
| `Lightweight` | Edge/Host | Simple text processing |
| `Network` | Host | HTTP requests |
| `FileIO` | Host | File operations |
| `Inference` | Accel | LLM inference |
| `Training` | Accel | Model fine-tuning |

## Usage

```rust
use agentvm_scheduler::{Scheduler, SchedulerConfig, NodeRegistry};
use agentvm_scheduler::node::{NodeInfo, Tier};
use agentvm_scheduler::task::{TaskSpec, TaskClass, CapsuleId};

// Create registry and scheduler
let registry = NodeRegistry::new();
registry.add(NodeInfo::new("host-01", Tier::Host)).await?;

let scheduler = Scheduler::new(SchedulerConfig::default(), registry);

// Schedule a task
let task = TaskSpec::new(CapsuleId::new(), TaskClass::Network);
let placement = scheduler.schedule(&task).await?;
```

## License

Apache-2.0
