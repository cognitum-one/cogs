# agentvm-types

Core types for Agentic VM - `no_std` compatible.

## Overview

This crate provides the foundational types shared across all tiers of the
Agentic VM system. It is designed to be `no_std` compatible for use on
edge microcontrollers.

## Types

- **Capsule**: `CapsuleId`, `CapsuleManifest`, `CapsuleIdentity`
- **Capability**: `CapabilityId`, `CapabilityType`, `Capability`, `Rights`, `Quota`
- **Budget**: `Budget`, `BudgetVector`
- **Evidence**: `EvidenceBundle`, `EvidenceStatement`
- **Error**: `AgentVmError`, `Result`

## Features

- `std` - Enable standard library support
- `alloc` - Enable alloc-only features (no_std compatible)
- `serde` - Enable serialization/deserialization

## Usage

```rust
use agentvm_types::{CapsuleId, CapabilityType, Budget};

let capsule_id = CapsuleId::generate();
let budget = Budget::default();
```

## License

Apache-2.0
