# agentvm-proxy

Capability proxy for Agentic VM - manages capsule access to external resources.

## Overview

This crate implements the capability proxy that mediates all capsule access
to external resources. It provides:

- Capability validation before each operation
- Evidence logging for all invocations
- Budget tracking and enforcement
- Network, filesystem, process, and secret executors

## Architecture

```
+----------+     +-------+     +----------+
| Capsule  | --> | Proxy | --> | Resource |
| (Guest)  |     |       |     | (Host)   |
+----------+     +---+---+     +----------+
                     |
                     v
              +-----------+
              | Evidence  |
              |   Log     |
              +-----------+
```

## Modules

- `config` - Proxy configuration
- `error` - Error types
- `evidence` - Evidence logging
- `wire` - Wire protocol handling
- `executor/` - Resource executors
  - `network` - HTTP/TCP execution
  - `filesystem` - File read/write
  - `process` - Process spawning
  - `secrets` - Secret retrieval

## Executors

| Executor | Capabilities | Description |
|----------|-------------|-------------|
| `NetworkExecutor` | NetworkHttp, NetworkTcp | HTTP requests, TCP connections |
| `FilesystemExecutor` | FileRead, FileWrite | File operations |
| `ProcessExecutor` | ProcessSpawn | Subprocess execution |
| `SecretsExecutor` | SecretRead | Environment secret access |

## Usage

```rust
use agentvm_proxy::{CapabilityProxy, ProxyConfig};

let config = ProxyConfig::default();
let proxy = CapabilityProxy::new(config).await?;

// Handle capability invocation
let response = proxy.handle_invoke(cap_id, request).await?;
```

## Features

- `mock` - Enable mock executors for testing

## License

Apache-2.0
