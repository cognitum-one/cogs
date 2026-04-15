# WASM/MCP Integration Guide

This guide covers running FXNN in the browser using WebAssembly and integrating with AI agents via the Model Context Protocol (MCP).

## Overview

FXNN provides two WASM interfaces:

1. **Direct API** - `WasmSimulation` for programmatic control
2. **MCP Protocol** - `McpHandler` for AI agent integration

## Building for WASM

### Prerequisites

```bash
# Install wasm-pack
cargo install wasm-pack

# Install wasm32 target
rustup target add wasm32-unknown-unknown
```

### Build

```bash
cd crates/fxnn
wasm-pack build --target web --features wasm
```

This creates a `pkg/` directory with the WASM module and JavaScript bindings.

## Direct API Usage

### JavaScript/TypeScript

```javascript
import init, { WasmSimulation, wasm_init } from './pkg/fxnn.js';

async function runSimulation() {
    // Initialize WASM module
    await init();
    wasm_init();  // Enable panic hooks for debugging

    // Create simulation with 256 atoms (4x4x4x4 FCC)
    const sim = WasmSimulation.new_fcc(4, 4, 4, 1.5, 1.0);

    // Configure
    sim.set_timestep(0.001);

    // Run 1000 steps
    sim.run(1000);

    // Get data
    console.log(`Step: ${sim.get_step()}`);
    console.log(`Temperature: ${sim.get_temperature()}`);
    console.log(`Total Energy: ${sim.get_total_energy()}`);

    // Get positions for visualization
    const positions = sim.get_positions();  // Float32Array
    console.log(`${positions.length / 3} atoms`);
}
```

### React Example

```tsx
import { useEffect, useState } from 'react';
import init, { WasmSimulation } from 'fxnn';

function SimulationViewer() {
    const [sim, setSim] = useState<WasmSimulation | null>(null);
    const [energy, setEnergy] = useState(0);

    useEffect(() => {
        init().then(() => {
            const s = WasmSimulation.new_fcc(4, 4, 4, 1.5, 1.0);
            setSim(s);
            setEnergy(s.get_total_energy());
        });
    }, []);

    const step = () => {
        if (sim) {
            sim.run(100);
            setEnergy(sim.get_total_energy());
        }
    };

    return (
        <div>
            <p>Energy: {energy.toFixed(4)}</p>
            <button onClick={step}>Run 100 Steps</button>
        </div>
    );
}
```

## MCP Protocol Usage

The MCP handler allows AI agents to interact with simulations using JSON-RPC 2.0.

### Initialize MCP Handler

```javascript
import init, { McpHandler } from 'fxnn';

async function setupMcp() {
    await init();
    const mcp = new McpHandler();

    // Check server info
    console.log(mcp.get_server_info());
    // {
    //   "name": "fxnn-mcp",
    //   "version": "0.1.0",
    //   "capabilities": { "tools": true, "resources": true }
    // }

    return mcp;
}
```

### MCP Tools

#### List Available Tools

```javascript
const response = mcp.handle_request(JSON.stringify({
    jsonrpc: "2.0",
    id: 1,
    method: "tools/list"
}));
// Returns list of all available tools with schemas
```

#### Create Simulation

```javascript
const response = mcp.handle_request(JSON.stringify({
    jsonrpc: "2.0",
    id: 2,
    method: "tools/call",
    params: {
        name: "simulation.create",
        arguments: {
            lattice_type: "fcc",  // or "random"
            nx: 4, ny: 4, nz: 4,
            spacing: 1.5,
            temperature: 1.0
        }
    }
}));
// Returns: { sim_id: "sim_0", n_atoms: 256, ... }
```

#### Run Simulation Steps

```javascript
const response = mcp.handle_request(JSON.stringify({
    jsonrpc: "2.0",
    id: 3,
    method: "tools/call",
    params: {
        name: "simulation.step",
        arguments: {
            sim_id: "sim_0",
            steps: 1000
        }
    }
}));
```

#### Get State

```javascript
const response = mcp.handle_request(JSON.stringify({
    jsonrpc: "2.0",
    id: 4,
    method: "tools/call",
    params: {
        name: "simulation.state",
        arguments: { sim_id: "sim_0" }
    }
}));
```

### MCP Resources

Resources provide read-only data access.

#### List Resources

```javascript
const response = mcp.handle_request(JSON.stringify({
    jsonrpc: "2.0",
    id: 5,
    method: "resources/list"
}));
```

#### Read Configuration

```javascript
// Get default parameters
const response = mcp.handle_request(JSON.stringify({
    jsonrpc: "2.0",
    id: 6,
    method: "resources/read",
    params: { uri: "fxnn://config/defaults" }
}));

// Get available force fields
const response = mcp.handle_request(JSON.stringify({
    jsonrpc: "2.0",
    id: 7,
    method: "resources/read",
    params: { uri: "fxnn://config/forcefields" }
}));
```

#### Read Simulation Data

```javascript
// Get full simulation state
const response = mcp.handle_request(JSON.stringify({
    jsonrpc: "2.0",
    id: 8,
    method: "resources/read",
    params: { uri: "fxnn://simulation/sim_0/state" }
}));

// Get just positions
const response = mcp.handle_request(JSON.stringify({
    jsonrpc: "2.0",
    id: 9,
    method: "resources/read",
    params: { uri: "fxnn://simulation/sim_0/positions" }
}));
```

## AI Agent Integration

### Claude/Anthropic Integration

```javascript
// Example: Claude can call FXNN tools
const claude_tool_call = {
    jsonrpc: "2.0",
    id: "claude-1",
    method: "tools/call",
    params: {
        name: "simulation.create",
        arguments: {
            lattice_type: "fcc",
            nx: 3, ny: 3, nz: 3,
            temperature: 300  // Can use real units
        }
    }
};

const result = mcp.handle_request(JSON.stringify(claude_tool_call));
```

### Error Handling

```javascript
const response = JSON.parse(mcp.handle_request(request));

if (response.error) {
    console.error(`Error ${response.error.code}: ${response.error.message}`);
} else {
    console.log('Result:', response.result);
}
```

## Performance Tips

1. **Batch steps**: Run multiple steps in one call (`steps: 1000`)
2. **Minimize data transfer**: Only read positions/velocities when needed for visualization
3. **Use typed arrays**: `get_positions()` returns `Float32Array` for efficient rendering

## Next Steps

- [Getting Started](getting-started.md) - Rust API basics
- [Force Fields](force-fields.md) - Available force field types
- [ADR-001](../adr/ADR-001-five-layer-reality-stack.md) - Architecture documentation
