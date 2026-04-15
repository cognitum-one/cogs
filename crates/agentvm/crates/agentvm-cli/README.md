# agentvm-cli

Command-line interface for Agentic VM - Accountable Agent Runtime.

## Overview

The `agentvm` CLI provides commands for:

- Running agents in capsules with evidence generation
- Managing snapshots for recovery
- Querying and verifying evidence
- Managing capabilities
- Running benchmarks

## Installation

```bash
cargo install --path .
```

## Commands

### Run

Execute a command in an agent capsule:

```bash
agentvm run --evidence --workspace ./project -- claude code
```

### Snapshot

Manage capsule snapshots:

```bash
agentvm snapshot create --name "before-refactor"
agentvm snapshot list
agentvm snapshot delete <id>
```

### Evidence

Query and verify evidence:

```bash
agentvm evidence get <run-id>
agentvm evidence query --capsule my-agent --start 2024-01-01
agentvm evidence verify ./evidence.json
agentvm evidence export <run-id> --format json --output ./export.json
```

### Capability

Manage capabilities:

```bash
agentvm capability list --capsule my-agent
agentvm capability grant --capsule my-agent --cap-type network --scope "https://api.example.com/*"
agentvm capability revoke <cap-id> --cascade
```

### Benchmark

Run and analyze benchmarks:

```bash
agentvm benchmark run --task "code-review" --iterations 30
agentvm benchmark verify ./results.json --p95-improvement 2.0
```

### Reset

Restore capsule to a snapshot:

```bash
agentvm reset --from-snapshot <snapshot-id> --preserve-workspace
```

### Replay

Replay execution from evidence:

```bash
agentvm replay ./evidence.json --verify-effects
```

## Configuration

Configuration can be provided via:
- `--config` flag
- `AGENTVM_CONFIG` environment variable
- `~/.config/agentvm/config.toml`

## License

Apache-2.0
