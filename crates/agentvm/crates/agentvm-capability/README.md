# agentvm-capability

Capability protocol implementation for Agentic VM - `no_std` compatible.

## Overview

This crate provides the wire protocol and validation logic for capability
tokens. It implements Ed25519-signed capability tokens with:

- Scope-limited access (hosts, paths, ports)
- Time-based expiry
- Quota tracking
- Derivation (attenuation)

## Modules

- `token` - Capability token creation and signing
- `derive` - Capability derivation (no amplification)
- `validate` - Capability validation against operations
- `wire` - Binary wire protocol (42-byte header)
- `scope` - Scope matching for network and filesystem

## Wire Protocol

```
+--------+--------+--------+--------+
|             Magic (4)             |  0x43415056 ("CAPV")
+--------+--------+--------+--------+
| Version (2)    | Flags (2)        |
+--------+--------+--------+--------+
|           Sequence (8)            |
+--------+--------+--------+--------+
|        Capability ID (16)         |
+--------+--------+--------+--------+
| Msg Type (2)  | Reserved (2)      |
+--------+--------+--------+--------+
|         Payload Length (4)        |
+--------+--------+--------+--------+
|            Payload (N)            |
+--------+--------+--------+--------+
|           CRC32C (4)              |
+--------+--------+--------+--------+
```

## Features

- `std` - Enable standard library support
- `serde-support` - Enable serialization

## License

Apache-2.0
