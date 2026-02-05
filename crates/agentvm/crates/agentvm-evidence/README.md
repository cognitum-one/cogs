# agentvm-evidence

Evidence chain implementation for Agentic VM - `no_std` compatible.

## Overview

This crate provides tamper-evident, cryptographically-signed evidence bundles
for tracking agent execution. It implements:

- Append-only Merkle tree
- DSSE (Dead Simple Signing Envelope) format
- Inclusion and consistency proofs
- Chain integrity verification

## Modules

- `bundle` - Evidence bundle construction
- `merkle` - Merkle tree operations
- `sign` - Ed25519 signing
- `statement` - Evidence statement types
- `verify` - Bundle and chain verification

## Evidence Bundle Structure

```json
{
  "_type": "https://agentvm.io/EvidenceStatement/v1",
  "header": {
    "run_id": "...",
    "capsule_id": "...",
    "timestamp_ns": 1234567890
  },
  "inputs": {
    "manifest_hash": "sha256:...",
    "workspace_hash": "sha256:..."
  },
  "execution": {
    "capability_calls": [...],
    "budget_consumed": {...}
  },
  "outputs": {
    "exit_code": 0,
    "workspace_diff_hash": "sha256:..."
  },
  "chain": {
    "previous_hash": "sha256:...",
    "merkle_root": "sha256:..."
  }
}
```

## Features

- `std` - Enable standard library support
- `serde` - Enable serialization
- `hsm` - Hardware security module support
- `tpm` - TPM signing support
- `rekor` - Rekor transparency log integration

## License

Apache-2.0
