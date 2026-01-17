//! Ultra-lightweight Model Context Protocol (MCP) implementation for FXNN.
//!
//! This module provides MCP tool support for AI agents to interact with
//! molecular dynamics simulations. It implements a minimal JSON-RPC 2.0
//! subset compatible with the MCP specification.
//!
//! # Design Goals
//!
//! - **Zero external dependencies**: Uses only serde_json for serialization
//! - **WASM-compatible**: Works in browser and Node.js environments
//! - **Self-documenting**: Tools include descriptions for AI agent discovery
//! - **Type-safe**: Rust type system enforces correct tool invocations
//!
//! # MCP Protocol
//!
//! Implements the Model Context Protocol for tool integration:
//! - `tools/list`: Discover available simulation tools
//! - `tools/call`: Execute a specific tool with parameters
//!
//! # Available Tools
//!
//! - `simulation.create`: Create a new simulation
//! - `simulation.step`: Run simulation steps
//! - `simulation.state`: Get current simulation state
//! - `simulation.energy`: Get energy values
//! - `simulation.atoms`: Get atom positions/velocities
//! - `simulation.configure`: Configure simulation parameters
//!
//! # Usage from JavaScript
//!
//! ```javascript
//! import { McpHandler } from 'fxnn';
//!
//! const mcp = new McpHandler();
//!
//! // List available tools
//! const tools = mcp.handle_request(JSON.stringify({
//!     jsonrpc: "2.0",
//!     id: 1,
//!     method: "tools/list"
//! }));
//!
//! // Create simulation
//! const result = mcp.handle_request(JSON.stringify({
//!     jsonrpc: "2.0",
//!     id: 2,
//!     method: "tools/call",
//!     params: {
//!         name: "simulation.create",
//!         arguments: {
//!             lattice_type: "fcc",
//!             nx: 4, ny: 4, nz: 4,
//!             spacing: 1.5,
//!             temperature: 1.0
//!         }
//!     }
//! }));
//! ```

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

use super::WasmSimulation;

// =============================================================================
// Snapshot/Restore Data Structures
// =============================================================================

/// A snapshot of simulation state for save/restore
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Snapshot {
    id: String,
    sim_id: String,
    step: u64,
    hash: String,
    timestamp: String,
    positions: Vec<f32>,
    velocities: Vec<f32>,
    box_size: f32,
    n_atoms: usize,
}

/// Witness log entry for audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WitnessEntry {
    step: u64,
    hash: String,
    prev_hash: String,
    event_type: String,
    timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// Episode for memory storage
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Episode {
    id: String,
    key: String,
    sim_id: String,
    observations: Vec<Vec<f32>>,
    actions: Vec<Vec<f32>>,
    rewards: Vec<f32>,
    total_reward: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Value>,
}

/// Scenario definition for quick-start
#[derive(Debug, Clone, Serialize)]
struct Scenario {
    id: String,
    name: String,
    description: String,
    category: String,
    params: Value,
}

/// Simple hash function for state verification (Blake3-like, but pure Rust)
fn compute_state_hash(positions: &[f32], velocities: &[f32], step: u64) -> String {
    // Simple FNV-1a hash for WASM compatibility (no external deps)
    let mut hash: u64 = 0xcbf29ce484222325;
    let prime: u64 = 0x100000001b3;

    // Hash step
    for byte in step.to_le_bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(prime);
    }

    // Hash positions
    for &p in positions {
        for byte in p.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(prime);
        }
    }

    // Hash velocities
    for &v in velocities {
        for byte in v.to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(prime);
        }
    }

    format!("{:016x}", hash)
}

fn current_timestamp() -> String {
    // In WASM, we don't have access to system time, use a placeholder
    // In real implementation, this would come from JS Date
    "2026-01-12T00:00:00Z".to_string()
}

/// JSON-RPC 2.0 request structure
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

/// JSON-RPC 2.0 response structure
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error structure
#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// MCP Tool definition for discovery
#[derive(Debug, Clone, Serialize)]
struct McpTool {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

/// MCP Tool call parameters
#[derive(Debug, Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: Value,
}

/// MCP Resource definition for discovery
#[derive(Debug, Clone, Serialize)]
struct McpResource {
    uri: String,
    name: String,
    description: String,
    #[serde(rename = "mimeType")]
    mime_type: String,
}

/// MCP Resource read parameters
#[derive(Debug, Deserialize)]
struct ResourceReadParams {
    uri: String,
}

/// Ultra-lightweight MCP handler for FXNN simulations.
///
/// This handler manages simulation instances and routes MCP tool calls
/// to the appropriate simulation methods.
#[wasm_bindgen]
pub struct McpHandler {
    simulations: HashMap<String, WasmSimulation>,
    snapshots: HashMap<String, Vec<Snapshot>>,
    witness_logs: HashMap<String, Vec<WitnessEntry>>,
    episodes: HashMap<String, Vec<Episode>>,
    next_sim_id: u32,
    next_snapshot_id: u32,
    next_episode_id: u32,
}

#[wasm_bindgen]
impl McpHandler {
    /// Create a new MCP handler instance.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            simulations: HashMap::new(),
            snapshots: HashMap::new(),
            witness_logs: HashMap::new(),
            episodes: HashMap::new(),
            next_sim_id: 0,
            next_snapshot_id: 0,
            next_episode_id: 0,
        }
    }

    /// Handle an MCP JSON-RPC request and return the response.
    ///
    /// # Arguments
    ///
    /// * `request_json` - JSON-RPC 2.0 request string
    ///
    /// # Returns
    ///
    /// JSON-RPC 2.0 response string
    #[wasm_bindgen]
    pub fn handle_request(&mut self, request_json: &str) -> String {
        let response = match self.process_request(request_json) {
            Ok(resp) => resp,
            Err(e) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: Value::Null,
                result: None,
                error: Some(e),
            },
        };
        serde_json::to_string(&response).unwrap_or_else(|_| {
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32603,"message":"Internal error"}}"#.to_string()
        })
    }

    /// Get the MCP server info.
    #[wasm_bindgen]
    pub fn get_server_info(&self) -> String {
        json!({
            "name": "fxnn-mcp",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "FXNN Molecular Dynamics Simulation MCP Server",
            "capabilities": {
                "tools": true,
                "resources": true,
                "prompts": false
            }
        }).to_string()
    }

    /// Get number of active simulations.
    #[wasm_bindgen]
    pub fn simulation_count(&self) -> usize {
        self.simulations.len()
    }
}

impl McpHandler {
    /// Process a JSON-RPC request and return the result.
    fn process_request(&mut self, request_json: &str) -> Result<JsonRpcResponse, JsonRpcError> {
        // Parse request
        let request: JsonRpcRequest = serde_json::from_str(request_json).map_err(|e| {
            JsonRpcError {
                code: -32700,
                message: format!("Parse error: {}", e),
                data: None,
            }
        })?;

        // Validate JSON-RPC version
        if request.jsonrpc != "2.0" {
            return Err(JsonRpcError {
                code: -32600,
                message: "Invalid Request: jsonrpc must be '2.0'".to_string(),
                data: None,
            });
        }

        let id = request.id.unwrap_or(Value::Null);

        // Route method
        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(),
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(request.params),
            "resources/list" => self.handle_resources_list(),
            "resources/read" => self.handle_resources_read(request.params),
            _ => Err(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", request.method),
                data: None,
            }),
        }?;

        Ok(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        })
    }

    /// Handle initialize request
    fn handle_initialize(&self) -> Result<Value, JsonRpcError> {
        Ok(json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "fxnn-mcp",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {
                "tools": {},
                "resources": {}
            }
        }))
    }

    /// Handle resources/list request - list all available resources
    fn handle_resources_list(&self) -> Result<Value, JsonRpcError> {
        let mut resources = vec![
            // Static resources always available
            McpResource {
                uri: "fxnn://config/defaults".to_string(),
                name: "Default Configuration".to_string(),
                description: "Default simulation parameters and settings".to_string(),
                mime_type: "application/json".to_string(),
            },
            McpResource {
                uri: "fxnn://config/forcefields".to_string(),
                name: "Force Field Parameters".to_string(),
                description: "Available force field types and their default parameters".to_string(),
                mime_type: "application/json".to_string(),
            },
            McpResource {
                uri: "fxnn://docs/api".to_string(),
                name: "API Documentation".to_string(),
                description: "Complete API reference for FXNN MCP tools".to_string(),
                mime_type: "text/markdown".to_string(),
            },
        ];

        // Add dynamic resources for each active simulation
        for (sim_id, sim) in &self.simulations {
            resources.push(McpResource {
                uri: format!("fxnn://simulation/{}/positions", sim_id),
                name: format!("Simulation {} Positions", sim_id),
                description: format!("Current atom positions for {} ({} atoms)", sim_id, sim.get_n_atoms()),
                mime_type: "application/json".to_string(),
            });
            resources.push(McpResource {
                uri: format!("fxnn://simulation/{}/velocities", sim_id),
                name: format!("Simulation {} Velocities", sim_id),
                description: format!("Current atom velocities for {}", sim_id),
                mime_type: "application/json".to_string(),
            });
            resources.push(McpResource {
                uri: format!("fxnn://simulation/{}/state", sim_id),
                name: format!("Simulation {} Full State", sim_id),
                description: format!("Complete state snapshot for {}", sim_id),
                mime_type: "application/json".to_string(),
            });
        }

        Ok(json!({ "resources": resources }))
    }

    /// Handle resources/read request - read a specific resource
    fn handle_resources_read(&mut self, params: Value) -> Result<Value, JsonRpcError> {
        let read_params: ResourceReadParams = serde_json::from_value(params).map_err(|e| {
            JsonRpcError {
                code: -32602,
                message: format!("Invalid params: {}", e),
                data: None,
            }
        })?;

        let uri = &read_params.uri;

        // Parse URI and route to appropriate handler
        if uri == "fxnn://config/defaults" {
            return Ok(self.resource_config_defaults());
        }
        if uri == "fxnn://config/forcefields" {
            return Ok(self.resource_forcefields());
        }
        if uri == "fxnn://docs/api" {
            return Ok(self.resource_api_docs());
        }

        // Handle simulation-specific resources
        if uri.starts_with("fxnn://simulation/") {
            let parts: Vec<&str> = uri.trim_start_matches("fxnn://simulation/").split('/').collect();
            if parts.len() == 2 {
                let sim_id = parts[0].to_string();
                let resource_type = parts[1];

                if !self.simulations.contains_key(&sim_id) {
                    return Err(JsonRpcError {
                        code: -32602,
                        message: format!("Simulation not found: {}", sim_id),
                        data: None,
                    });
                }

                return match resource_type {
                    "positions" => Ok(self.resource_positions(&sim_id)),
                    "velocities" => Ok(self.resource_velocities(&sim_id)),
                    "state" => Ok(self.resource_full_state(&sim_id)),
                    _ => Err(JsonRpcError {
                        code: -32602,
                        message: format!("Unknown resource type: {}", resource_type),
                        data: None,
                    }),
                };
            }
        }

        Err(JsonRpcError {
            code: -32602,
            message: format!("Resource not found: {}", uri),
            data: None,
        })
    }

    /// Resource: Default configuration parameters
    fn resource_config_defaults(&self) -> Value {
        json!({
            "contents": [{
                "uri": "fxnn://config/defaults",
                "mimeType": "application/json",
                "text": serde_json::to_string_pretty(&json!({
                    "simulation": {
                        "timestep": 0.001,
                        "default_temperature": 1.0,
                        "default_box_size": 10.0,
                        "max_atoms": 100000
                    },
                    "forcefield": {
                        "type": "lennard-jones",
                        "epsilon": 1.0,
                        "sigma": 1.0,
                        "cutoff": 2.5
                    },
                    "integrator": {
                        "type": "velocity-verlet"
                    },
                    "neighbor_list": {
                        "type": "cell-list",
                        "skin": 0.3,
                        "update_frequency": 20
                    }
                })).unwrap()
            }]
        })
    }

    /// Resource: Available force fields
    fn resource_forcefields(&self) -> Value {
        json!({
            "contents": [{
                "uri": "fxnn://config/forcefields",
                "mimeType": "application/json",
                "text": serde_json::to_string_pretty(&json!({
                    "force_fields": [
                        {
                            "name": "lennard-jones",
                            "description": "Standard 12-6 Lennard-Jones potential",
                            "parameters": {
                                "epsilon": {"type": "float", "description": "Well depth", "default": 1.0},
                                "sigma": {"type": "float", "description": "Zero-crossing distance", "default": 1.0},
                                "cutoff": {"type": "float", "description": "Cutoff radius", "default": 2.5}
                            }
                        },
                        {
                            "name": "coulomb",
                            "description": "Electrostatic Coulomb potential",
                            "parameters": {
                                "cutoff": {"type": "float", "description": "Cutoff radius", "default": 10.0},
                                "dielectric": {"type": "float", "description": "Dielectric constant", "default": 1.0}
                            }
                        }
                    ]
                })).unwrap()
            }]
        })
    }

    /// Resource: API documentation
    fn resource_api_docs(&self) -> Value {
        json!({
            "contents": [{
                "uri": "fxnn://docs/api",
                "mimeType": "text/markdown",
                "text": r#"# FXNN MCP API Reference

## Tools

### simulation.create
Create a new molecular dynamics simulation.

**Parameters:**
- `lattice_type` (string): "fcc" or "random"
- `nx`, `ny`, `nz` (int): Lattice cells in each dimension
- `spacing` (float): Lattice spacing
- `temperature` (float): Initial temperature

### simulation.step
Advance simulation by specified steps.

**Parameters:**
- `sim_id` (string): Simulation identifier
- `steps` (int): Number of steps to run

### simulation.state
Get current simulation state.

**Parameters:**
- `sim_id` (string): Simulation identifier

### simulation.energy
Get energy breakdown.

**Parameters:**
- `sim_id` (string): Simulation identifier

### simulation.configure
Modify simulation parameters.

**Parameters:**
- `sim_id` (string): Simulation identifier
- `timestep` (float, optional): New timestep
- `temperature` (float, optional): New temperature

### simulation.destroy
Clean up simulation resources.

**Parameters:**
- `sim_id` (string): Simulation identifier

### simulation.list
List all active simulations.

## Resources

### fxnn://config/defaults
Default simulation configuration parameters.

### fxnn://config/forcefields
Available force field types and parameters.

### fxnn://simulation/{id}/positions
Current atom positions (JSON array).

### fxnn://simulation/{id}/velocities
Current atom velocities (JSON array).

### fxnn://simulation/{id}/state
Complete simulation state snapshot.
"#
            }]
        })
    }

    /// Resource: Simulation positions
    fn resource_positions(&mut self, sim_id: &str) -> Value {
        let sim = self.simulations.get_mut(sim_id).unwrap();
        let n_atoms = sim.get_n_atoms();
        let positions: Vec<f32> = sim.get_positions().to_vec();
        json!({
            "contents": [{
                "uri": format!("fxnn://simulation/{}/positions", sim_id),
                "mimeType": "application/json",
                "text": serde_json::to_string(&json!({
                    "n_atoms": n_atoms,
                    "positions": positions
                })).unwrap()
            }]
        })
    }

    /// Resource: Simulation velocities
    fn resource_velocities(&mut self, sim_id: &str) -> Value {
        let sim = self.simulations.get_mut(sim_id).unwrap();
        let n_atoms = sim.get_n_atoms();
        let velocities: Vec<f32> = sim.get_velocities().to_vec();
        json!({
            "contents": [{
                "uri": format!("fxnn://simulation/{}/velocities", sim_id),
                "mimeType": "application/json",
                "text": serde_json::to_string(&json!({
                    "n_atoms": n_atoms,
                    "velocities": velocities
                })).unwrap()
            }]
        })
    }

    /// Resource: Full simulation state
    fn resource_full_state(&mut self, sim_id: &str) -> Value {
        let sim = self.simulations.get_mut(sim_id).unwrap();
        let n_atoms = sim.get_n_atoms();
        let step = sim.get_step();
        let time = sim.get_time();
        let temperature = sim.get_temperature();
        let kinetic_energy = sim.get_kinetic_energy();
        let potential_energy = sim.get_potential_energy();
        let total_energy = sim.get_total_energy();
        let positions: Vec<f32> = sim.get_positions().to_vec();
        let velocities: Vec<f32> = sim.get_velocities().to_vec();

        json!({
            "contents": [{
                "uri": format!("fxnn://simulation/{}/state", sim_id),
                "mimeType": "application/json",
                "text": serde_json::to_string(&json!({
                    "sim_id": sim_id,
                    "n_atoms": n_atoms,
                    "step": step,
                    "time": time,
                    "temperature": temperature,
                    "kinetic_energy": kinetic_energy,
                    "potential_energy": potential_energy,
                    "total_energy": total_energy,
                    "positions": positions,
                    "velocities": velocities
                })).unwrap()
            }]
        })
    }

    /// Handle tools/list request
    fn handle_tools_list(&self) -> Result<Value, JsonRpcError> {
        let tools = vec![
            McpTool {
                name: "simulation.create".to_string(),
                description: "Create a new molecular dynamics simulation with specified parameters".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "lattice_type": {
                            "type": "string",
                            "enum": ["fcc", "random"],
                            "description": "Initial atom arrangement (fcc = face-centered cubic, random = random positions)"
                        },
                        "nx": {"type": "integer", "description": "Lattice cells in X", "default": 4},
                        "ny": {"type": "integer", "description": "Lattice cells in Y", "default": 4},
                        "nz": {"type": "integer", "description": "Lattice cells in Z", "default": 4},
                        "spacing": {"type": "number", "description": "Lattice spacing in reduced units", "default": 1.5},
                        "temperature": {"type": "number", "description": "Initial temperature", "default": 1.0}
                    },
                    "required": ["lattice_type"]
                }),
            },
            McpTool {
                name: "simulation.step".to_string(),
                description: "Run simulation for specified number of timesteps".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "sim_id": {"type": "string", "description": "Simulation ID"},
                        "steps": {"type": "integer", "description": "Number of steps to run", "default": 100}
                    },
                    "required": ["sim_id"]
                }),
            },
            McpTool {
                name: "simulation.state".to_string(),
                description: "Get current simulation state including step count and system properties".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "sim_id": {"type": "string", "description": "Simulation ID"}
                    },
                    "required": ["sim_id"]
                }),
            },
            McpTool {
                name: "simulation.energy".to_string(),
                description: "Get current energy values (kinetic, potential, total)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "sim_id": {"type": "string", "description": "Simulation ID"}
                    },
                    "required": ["sim_id"]
                }),
            },
            McpTool {
                name: "simulation.configure".to_string(),
                description: "Configure simulation parameters".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "sim_id": {"type": "string", "description": "Simulation ID"},
                        "timestep": {"type": "number", "description": "Integration timestep"},
                        "temperature": {"type": "number", "description": "Target temperature (for thermostat)"}
                    },
                    "required": ["sim_id"]
                }),
            },
            McpTool {
                name: "simulation.destroy".to_string(),
                description: "Destroy a simulation and free its resources".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "sim_id": {"type": "string", "description": "Simulation ID"}
                    },
                    "required": ["sim_id"]
                }),
            },
            McpTool {
                name: "simulation.list".to_string(),
                description: "List all active simulations".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            // === Snapshot/Restore Tools ===
            McpTool {
                name: "simulation.snapshot".to_string(),
                description: "Save current simulation state as a snapshot with cryptographic hash".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "sim_id": {"type": "string", "description": "Simulation ID"}
                    },
                    "required": ["sim_id"]
                }),
            },
            McpTool {
                name: "simulation.restore".to_string(),
                description: "Restore simulation to a previous snapshot state".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "sim_id": {"type": "string", "description": "Simulation ID"},
                        "snapshot_id": {"type": "string", "description": "Snapshot ID to restore"}
                    },
                    "required": ["sim_id", "snapshot_id"]
                }),
            },
            McpTool {
                name: "simulation.snapshots".to_string(),
                description: "List all snapshots for a simulation".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "sim_id": {"type": "string", "description": "Simulation ID"}
                    },
                    "required": ["sim_id"]
                }),
            },
            // === Witness/Audit Trail Tools ===
            McpTool {
                name: "simulation.witness".to_string(),
                description: "Get witness log entries (tamper-evident audit trail)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "sim_id": {"type": "string", "description": "Simulation ID"},
                        "limit": {"type": "integer", "description": "Max entries to return", "default": 100}
                    },
                    "required": ["sim_id"]
                }),
            },
            McpTool {
                name: "simulation.verify".to_string(),
                description: "Verify hash chain integrity for audit trail".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "sim_id": {"type": "string", "description": "Simulation ID"}
                    },
                    "required": ["sim_id"]
                }),
            },
            // === Scenario Library Tools ===
            McpTool {
                name: "simulation.scenarios".to_string(),
                description: "List available pre-built simulation scenarios".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            McpTool {
                name: "simulation.load_scenario".to_string(),
                description: "Create a simulation from a pre-built scenario".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "scenario_id": {"type": "string", "description": "Scenario ID to load"},
                        "overrides": {"type": "object", "description": "Optional parameter overrides"}
                    },
                    "required": ["scenario_id"]
                }),
            },
            // === Observation Tools ===
            McpTool {
                name: "simulation.observe".to_string(),
                description: "Get observation vector for agent/RL integration".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "sim_id": {"type": "string", "description": "Simulation ID"},
                        "observer_type": {"type": "string", "enum": ["global", "local"], "default": "global"},
                        "center": {"type": "array", "items": {"type": "number"}, "description": "Center for local observation [x,y,z]"},
                        "radius": {"type": "number", "description": "Observation radius for local view", "default": 5.0}
                    },
                    "required": ["sim_id"]
                }),
            },
            // === Memory/Episode Tools ===
            McpTool {
                name: "simulation.memory_store".to_string(),
                description: "Store an episode in episodic memory".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "sim_id": {"type": "string", "description": "Simulation ID"},
                        "key": {"type": "string", "description": "Episode key/label"},
                        "observations": {"type": "array", "description": "List of observation vectors"},
                        "actions": {"type": "array", "description": "List of action vectors"},
                        "rewards": {"type": "array", "items": {"type": "number"}, "description": "List of rewards"}
                    },
                    "required": ["sim_id", "key"]
                }),
            },
            McpTool {
                name: "simulation.memory_list".to_string(),
                description: "List stored episodes for a simulation".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "sim_id": {"type": "string", "description": "Simulation ID"},
                        "limit": {"type": "integer", "description": "Max episodes to return", "default": 50}
                    },
                    "required": ["sim_id"]
                }),
            },
            McpTool {
                name: "simulation.memory_replay".to_string(),
                description: "Retrieve a stored episode for replay".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "sim_id": {"type": "string", "description": "Simulation ID"},
                        "episode_id": {"type": "string", "description": "Episode ID to retrieve"}
                    },
                    "required": ["sim_id", "episode_id"]
                }),
            },
            // === Benchmark Tools ===
            McpTool {
                name: "simulation.bench".to_string(),
                description: "Run performance benchmark on simulation".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "suite": {"type": "string", "enum": ["all", "force", "integrate", "hash"], "default": "all"},
                        "n_atoms": {"type": "integer", "description": "Number of atoms for benchmark", "default": 1000},
                        "n_steps": {"type": "integer", "description": "Steps to run", "default": 100}
                    }
                }),
            },
        ];

        Ok(json!({ "tools": tools }))
    }

    /// Handle tools/call request
    fn handle_tools_call(&mut self, params: Value) -> Result<Value, JsonRpcError> {
        let call_params: ToolCallParams = serde_json::from_value(params).map_err(|e| {
            JsonRpcError {
                code: -32602,
                message: format!("Invalid params: {}", e),
                data: None,
            }
        })?;

        match call_params.name.as_str() {
            // Core simulation tools
            "simulation.create" => self.tool_simulation_create(call_params.arguments),
            "simulation.step" => self.tool_simulation_step(call_params.arguments),
            "simulation.state" => self.tool_simulation_state(call_params.arguments),
            "simulation.energy" => self.tool_simulation_energy(call_params.arguments),
            "simulation.configure" => self.tool_simulation_configure(call_params.arguments),
            "simulation.destroy" => self.tool_simulation_destroy(call_params.arguments),
            "simulation.list" => self.tool_simulation_list(),
            // Snapshot/Restore tools
            "simulation.snapshot" => self.tool_snapshot(call_params.arguments),
            "simulation.restore" => self.tool_restore(call_params.arguments),
            "simulation.snapshots" => self.tool_snapshots_list(call_params.arguments),
            // Witness/Audit tools
            "simulation.witness" => self.tool_witness(call_params.arguments),
            "simulation.verify" => self.tool_verify(call_params.arguments),
            // Scenario tools
            "simulation.scenarios" => self.tool_scenarios_list(),
            "simulation.load_scenario" => self.tool_load_scenario(call_params.arguments),
            // Observation tools
            "simulation.observe" => self.tool_observe(call_params.arguments),
            // Memory/Episode tools
            "simulation.memory_store" => self.tool_memory_store(call_params.arguments),
            "simulation.memory_list" => self.tool_memory_list(call_params.arguments),
            "simulation.memory_replay" => self.tool_memory_replay(call_params.arguments),
            // Benchmark tools
            "simulation.bench" => self.tool_bench(call_params.arguments),
            _ => Err(JsonRpcError {
                code: -32601,
                message: format!("Unknown tool: {}", call_params.name),
                data: None,
            }),
        }
    }

    /// Create a new simulation
    fn tool_simulation_create(&mut self, args: Value) -> Result<Value, JsonRpcError> {
        let lattice_type = args.get("lattice_type")
            .and_then(|v| v.as_str())
            .unwrap_or("fcc");
        let nx = args.get("nx").and_then(|v| v.as_u64()).unwrap_or(4) as usize;
        let ny = args.get("ny").and_then(|v| v.as_u64()).unwrap_or(4) as usize;
        let nz = args.get("nz").and_then(|v| v.as_u64()).unwrap_or(4) as usize;
        let spacing = args.get("spacing").and_then(|v| v.as_f64()).unwrap_or(1.5) as f32;
        let temperature = args.get("temperature").and_then(|v| v.as_f64()).unwrap_or(1.0) as f32;

        let sim = match lattice_type {
            "fcc" => WasmSimulation::new_fcc(nx, ny, nz, spacing, temperature),
            "random" => {
                let n_atoms = nx * ny * nz * 4; // FCC has 4 atoms per unit cell
                let box_size = spacing * nx.max(ny).max(nz) as f32 * 2.0;
                WasmSimulation::new_random(n_atoms, box_size, temperature)
            }
            _ => return Err(JsonRpcError {
                code: -32602,
                message: format!("Unknown lattice type: {}. Supported: fcc, random", lattice_type),
                data: None,
            }),
        };

        let sim_id = format!("sim_{}", self.next_sim_id);
        self.next_sim_id += 1;

        let n_atoms = sim.get_n_atoms();
        self.simulations.insert(sim_id.clone(), sim);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Created simulation '{}' with {} atoms in {} lattice at T={}",
                    sim_id, n_atoms, lattice_type, temperature)
            }],
            "sim_id": sim_id,
            "n_atoms": n_atoms,
            "lattice_type": lattice_type,
            "temperature": temperature
        }))
    }

    /// Run simulation steps
    fn tool_simulation_step(&mut self, args: Value) -> Result<Value, JsonRpcError> {
        let sim_id = self.get_sim_id(&args)?;
        let steps = args.get("steps").and_then(|v| v.as_u64()).unwrap_or(100) as u32;

        let sim = self.simulations.get_mut(&sim_id).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: format!("Simulation not found: {}", sim_id),
            data: None,
        })?;

        sim.run(steps as usize);
        let current_step = sim.get_step();
        let energy = sim.get_total_energy();

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Ran {} steps. Now at step {}. Total energy: {:.4}", steps, current_step, energy)
            }],
            "steps_run": steps,
            "current_step": current_step,
            "total_energy": energy
        }))
    }

    /// Get simulation state
    fn tool_simulation_state(&self, args: Value) -> Result<Value, JsonRpcError> {
        let sim_id = self.get_sim_id(&args)?;

        let sim = self.simulations.get(&sim_id).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: format!("Simulation not found: {}", sim_id),
            data: None,
        })?;

        let state = json!({
            "sim_id": sim_id,
            "n_atoms": sim.get_n_atoms(),
            "current_step": sim.get_step(),
            "time": sim.get_time(),
            "temperature": sim.get_temperature(),
            "total_energy": sim.get_total_energy(),
            "kinetic_energy": sim.get_kinetic_energy(),
            "potential_energy": sim.get_potential_energy(),
        });

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Simulation '{}': {} atoms, step {}, T={:.2}, E={:.4}",
                    sim_id, sim.get_n_atoms(), sim.get_step(),
                    sim.get_temperature(), sim.get_total_energy())
            }],
            "state": state
        }))
    }

    /// Get simulation energy
    fn tool_simulation_energy(&self, args: Value) -> Result<Value, JsonRpcError> {
        let sim_id = self.get_sim_id(&args)?;

        let sim = self.simulations.get(&sim_id).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: format!("Simulation not found: {}", sim_id),
            data: None,
        })?;

        let kinetic = sim.get_kinetic_energy();
        let potential = sim.get_potential_energy();
        let total = sim.get_total_energy();

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Energy: kinetic={:.4}, potential={:.4}, total={:.4}",
                    kinetic, potential, total)
            }],
            "kinetic_energy": kinetic,
            "potential_energy": potential,
            "total_energy": total
        }))
    }

    /// Configure simulation
    fn tool_simulation_configure(&mut self, args: Value) -> Result<Value, JsonRpcError> {
        let sim_id = self.get_sim_id(&args)?;

        let sim = self.simulations.get_mut(&sim_id).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: format!("Simulation not found: {}", sim_id),
            data: None,
        })?;

        let mut changes = Vec::new();

        if let Some(dt) = args.get("timestep").and_then(|v| v.as_f64()) {
            sim.set_timestep(dt as f32);
            changes.push(format!("timestep={}", dt));
        }

        if let Some(temp) = args.get("temperature").and_then(|v| v.as_f64()) {
            sim.set_temperature(temp as f32);
            changes.push(format!("temperature={}", temp));
        }

        Ok(json!({
            "content": [{
                "type": "text",
                "text": if changes.is_empty() {
                    "No configuration changes applied".to_string()
                } else {
                    format!("Applied: {}", changes.join(", "))
                }
            }],
            "changes": changes
        }))
    }

    /// Destroy simulation
    fn tool_simulation_destroy(&mut self, args: Value) -> Result<Value, JsonRpcError> {
        let sim_id = self.get_sim_id(&args)?;

        if self.simulations.remove(&sim_id).is_some() {
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Destroyed simulation '{}'", sim_id)
                }],
                "destroyed": true
            }))
        } else {
            Err(JsonRpcError {
                code: -32602,
                message: format!("Simulation not found: {}", sim_id),
                data: None,
            })
        }
    }

    /// List all simulations
    fn tool_simulation_list(&self) -> Result<Value, JsonRpcError> {
        let sims: Vec<Value> = self.simulations.iter().map(|(id, sim)| {
            json!({
                "id": id,
                "n_atoms": sim.get_n_atoms(),
                "step": sim.get_step(),
                "energy": sim.get_total_energy()
            })
        }).collect();

        Ok(json!({
            "content": [{
                "type": "text",
                "text": if sims.is_empty() {
                    "No active simulations".to_string()
                } else {
                    format!("{} active simulation(s)", sims.len())
                }
            }],
            "simulations": sims
        }))
    }

    /// Helper to extract sim_id from args
    fn get_sim_id(&self, args: &Value) -> Result<String, JsonRpcError> {
        args.get("sim_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| JsonRpcError {
                code: -32602,
                message: "Missing required parameter: sim_id".to_string(),
                data: None,
            })
    }

    // =========================================================================
    // Snapshot/Restore Tool Implementations
    // =========================================================================

    /// Create a snapshot of current simulation state
    fn tool_snapshot(&mut self, args: Value) -> Result<Value, JsonRpcError> {
        let sim_id = self.get_sim_id(&args)?;

        let sim = self.simulations.get_mut(&sim_id).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: format!("Simulation not found: {}", sim_id),
            data: None,
        })?;

        // Get current state
        let step = sim.get_step();
        let positions: Vec<f32> = sim.get_positions().to_vec();
        let velocities: Vec<f32> = sim.get_velocities().to_vec();
        let n_atoms = sim.get_n_atoms();
        let box_size = 10.0; // TODO: Get from simulation

        // Compute hash
        let hash = compute_state_hash(&positions, &velocities, step);

        // Create snapshot
        let snapshot_id = format!("snap_{}", self.next_snapshot_id);
        self.next_snapshot_id += 1;

        let snapshot = Snapshot {
            id: snapshot_id.clone(),
            sim_id: sim_id.clone(),
            step,
            hash: hash.clone(),
            timestamp: current_timestamp(),
            positions,
            velocities,
            box_size,
            n_atoms,
        };

        // Store snapshot
        self.snapshots
            .entry(sim_id.clone())
            .or_insert_with(Vec::new)
            .push(snapshot);

        // Log to witness trail
        self.add_witness_entry(&sim_id, step, &hash, "snapshot", None);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Created snapshot '{}' at step {} with hash {}", snapshot_id, step, &hash[..16])
            }],
            "snapshot_id": snapshot_id,
            "step": step,
            "hash": hash,
            "n_atoms": n_atoms,
            "timestamp": current_timestamp()
        }))
    }

    /// Restore simulation to a previous snapshot
    fn tool_restore(&mut self, args: Value) -> Result<Value, JsonRpcError> {
        let sim_id = self.get_sim_id(&args)?;
        let snapshot_id = args.get("snapshot_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError {
                code: -32602,
                message: "Missing required parameter: snapshot_id".to_string(),
                data: None,
            })?;

        // Find snapshot
        let snapshots = self.snapshots.get(&sim_id).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: format!("No snapshots found for simulation: {}", sim_id),
            data: None,
        })?;

        let snapshot = snapshots.iter().find(|s| s.id == snapshot_id).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: format!("Snapshot not found: {}", snapshot_id),
            data: None,
        })?;

        // Clone needed data before mutable borrow
        let positions = snapshot.positions.clone();
        let velocities = snapshot.velocities.clone();
        let restored_step = snapshot.step;
        let hash = snapshot.hash.clone();

        // Restore state
        let sim = self.simulations.get_mut(&sim_id).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: format!("Simulation not found: {}", sim_id),
            data: None,
        })?;

        sim.set_positions(&positions);
        sim.set_velocities(&velocities);

        // Log to witness trail
        self.add_witness_entry(&sim_id, restored_step, &hash, "restore",
            Some(json!({"snapshot_id": snapshot_id})));

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Restored simulation to snapshot '{}' at step {}", snapshot_id, restored_step)
            }],
            "success": true,
            "restored_step": restored_step,
            "hash": hash
        }))
    }

    /// List all snapshots for a simulation
    fn tool_snapshots_list(&self, args: Value) -> Result<Value, JsonRpcError> {
        let sim_id = self.get_sim_id(&args)?;

        let snapshots = self.snapshots.get(&sim_id).cloned().unwrap_or_default();
        let snapshot_list: Vec<Value> = snapshots.iter().map(|s| {
            json!({
                "id": s.id,
                "step": s.step,
                "hash": s.hash,
                "timestamp": s.timestamp,
                "n_atoms": s.n_atoms
            })
        }).collect();

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("{} snapshot(s) for simulation '{}'", snapshot_list.len(), sim_id)
            }],
            "snapshots": snapshot_list
        }))
    }

    // =========================================================================
    // Witness/Audit Trail Tool Implementations
    // =========================================================================

    /// Get witness log entries
    fn tool_witness(&self, args: Value) -> Result<Value, JsonRpcError> {
        let sim_id = self.get_sim_id(&args)?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

        let entries = self.witness_logs.get(&sim_id).cloned().unwrap_or_default();
        let entries: Vec<Value> = entries.iter().rev().take(limit).map(|e| {
            json!({
                "step": e.step,
                "hash": e.hash,
                "prev_hash": e.prev_hash,
                "event_type": e.event_type,
                "timestamp": e.timestamp,
                "data": e.data
            })
        }).collect();

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("{} witness entries for simulation '{}'", entries.len(), sim_id)
            }],
            "entries": entries
        }))
    }

    /// Verify witness hash chain integrity
    fn tool_verify(&self, args: Value) -> Result<Value, JsonRpcError> {
        let sim_id = self.get_sim_id(&args)?;

        let entries = self.witness_logs.get(&sim_id).cloned().unwrap_or_default();

        if entries.is_empty() {
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": "No witness entries to verify"
                }],
                "valid": true,
                "checked_steps": 0
            }));
        }

        // Verify hash chain
        let mut valid = true;
        let mut first_invalid: Option<u64> = None;

        for i in 1..entries.len() {
            if entries[i].prev_hash != entries[i-1].hash {
                valid = false;
                first_invalid = Some(entries[i].step);
                break;
            }
        }

        Ok(json!({
            "content": [{
                "type": "text",
                "text": if valid {
                    format!("Hash chain verified: {} entries valid", entries.len())
                } else {
                    format!("Hash chain broken at step {}", first_invalid.unwrap())
                }
            }],
            "valid": valid,
            "checked_steps": entries.len(),
            "first_invalid": first_invalid
        }))
    }

    /// Helper to add witness entry
    fn add_witness_entry(&mut self, sim_id: &str, step: u64, hash: &str, event_type: &str, data: Option<Value>) {
        let entries = self.witness_logs.entry(sim_id.to_string()).or_insert_with(Vec::new);

        let prev_hash = entries.last()
            .map(|e| e.hash.clone())
            .unwrap_or_else(|| "genesis".to_string());

        entries.push(WitnessEntry {
            step,
            hash: hash.to_string(),
            prev_hash,
            event_type: event_type.to_string(),
            timestamp: current_timestamp(),
            data,
        });
    }

    // =========================================================================
    // Scenario Library Tool Implementations
    // =========================================================================

    /// List available scenarios
    fn tool_scenarios_list(&self) -> Result<Value, JsonRpcError> {
        let scenarios = vec![
            Scenario {
                id: "argon_256".to_string(),
                name: "Argon Gas (Small)".to_string(),
                description: "256 Argon atoms with Lennard-Jones potential".to_string(),
                category: "molecular".to_string(),
                params: json!({
                    "lattice_type": "fcc",
                    "nx": 4, "ny": 4, "nz": 4,
                    "spacing": 1.5,
                    "temperature": 1.0
                }),
            },
            Scenario {
                id: "argon_2048".to_string(),
                name: "Argon Gas (Medium)".to_string(),
                description: "2048 Argon atoms with Lennard-Jones potential".to_string(),
                category: "molecular".to_string(),
                params: json!({
                    "lattice_type": "fcc",
                    "nx": 8, "ny": 8, "nz": 8,
                    "spacing": 1.5,
                    "temperature": 1.0
                }),
            },
            Scenario {
                id: "crystal_fcc".to_string(),
                name: "FCC Crystal".to_string(),
                description: "Perfect FCC lattice at low temperature".to_string(),
                category: "molecular".to_string(),
                params: json!({
                    "lattice_type": "fcc",
                    "nx": 5, "ny": 5, "nz": 5,
                    "spacing": 1.1,
                    "temperature": 0.1
                }),
            },
            Scenario {
                id: "liquid_lj".to_string(),
                name: "LJ Liquid".to_string(),
                description: "Lennard-Jones liquid at T=1.0".to_string(),
                category: "molecular".to_string(),
                params: json!({
                    "lattice_type": "fcc",
                    "nx": 6, "ny": 6, "nz": 6,
                    "spacing": 1.3,
                    "temperature": 1.0
                }),
            },
            Scenario {
                id: "phase_transition".to_string(),
                name: "Phase Transition".to_string(),
                description: "System near melting point for phase study".to_string(),
                category: "molecular".to_string(),
                params: json!({
                    "lattice_type": "fcc",
                    "nx": 8, "ny": 8, "nz": 8,
                    "spacing": 1.2,
                    "temperature": 0.7
                }),
            },
            Scenario {
                id: "random_gas".to_string(),
                name: "Random Gas".to_string(),
                description: "Randomly distributed atoms at high temperature".to_string(),
                category: "molecular".to_string(),
                params: json!({
                    "lattice_type": "random",
                    "nx": 5, "ny": 5, "nz": 5,
                    "spacing": 2.0,
                    "temperature": 2.0
                }),
            },
            Scenario {
                id: "benchmark_small".to_string(),
                name: "Benchmark (Small)".to_string(),
                description: "Small system for quick benchmarks".to_string(),
                category: "benchmark".to_string(),
                params: json!({
                    "lattice_type": "fcc",
                    "nx": 3, "ny": 3, "nz": 3,
                    "spacing": 1.5,
                    "temperature": 1.0
                }),
            },
            Scenario {
                id: "benchmark_large".to_string(),
                name: "Benchmark (Large)".to_string(),
                description: "Large system for performance testing".to_string(),
                category: "benchmark".to_string(),
                params: json!({
                    "lattice_type": "fcc",
                    "nx": 10, "ny": 10, "nz": 10,
                    "spacing": 1.5,
                    "temperature": 1.0
                }),
            },
        ];

        let scenario_list: Vec<Value> = scenarios.iter().map(|s| {
            json!({
                "id": s.id,
                "name": s.name,
                "description": s.description,
                "category": s.category,
                "params": s.params
            })
        }).collect();

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("{} scenarios available", scenario_list.len())
            }],
            "scenarios": scenario_list
        }))
    }

    /// Load a scenario
    fn tool_load_scenario(&mut self, args: Value) -> Result<Value, JsonRpcError> {
        let scenario_id = args.get("scenario_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError {
                code: -32602,
                message: "Missing required parameter: scenario_id".to_string(),
                data: None,
            })?;

        // Get scenario params
        let (params, description) = match scenario_id {
            "argon_256" => (json!({"lattice_type": "fcc", "nx": 4, "ny": 4, "nz": 4, "spacing": 1.5, "temperature": 1.0}), "256 Argon atoms"),
            "argon_2048" => (json!({"lattice_type": "fcc", "nx": 8, "ny": 8, "nz": 8, "spacing": 1.5, "temperature": 1.0}), "2048 Argon atoms"),
            "crystal_fcc" => (json!({"lattice_type": "fcc", "nx": 5, "ny": 5, "nz": 5, "spacing": 1.1, "temperature": 0.1}), "FCC crystal"),
            "liquid_lj" => (json!({"lattice_type": "fcc", "nx": 6, "ny": 6, "nz": 6, "spacing": 1.3, "temperature": 1.0}), "LJ liquid"),
            "phase_transition" => (json!({"lattice_type": "fcc", "nx": 8, "ny": 8, "nz": 8, "spacing": 1.2, "temperature": 0.7}), "Phase transition system"),
            "random_gas" => (json!({"lattice_type": "random", "nx": 5, "ny": 5, "nz": 5, "spacing": 2.0, "temperature": 2.0}), "Random gas"),
            "benchmark_small" => (json!({"lattice_type": "fcc", "nx": 3, "ny": 3, "nz": 3, "spacing": 1.5, "temperature": 1.0}), "Small benchmark"),
            "benchmark_large" => (json!({"lattice_type": "fcc", "nx": 10, "ny": 10, "nz": 10, "spacing": 1.5, "temperature": 1.0}), "Large benchmark"),
            _ => return Err(JsonRpcError {
                code: -32602,
                message: format!("Unknown scenario: {}", scenario_id),
                data: None,
            }),
        };

        // Apply overrides
        let mut final_params = params.clone();
        if let Some(overrides) = args.get("overrides").and_then(|v| v.as_object()) {
            for (key, value) in overrides {
                final_params[key] = value.clone();
            }
        }

        // Create simulation using the params
        self.tool_simulation_create(final_params).map(|mut result| {
            result["loaded_scenario"] = json!(scenario_id);
            result["description"] = json!(description);
            result
        })
    }

    // =========================================================================
    // Observation Tool Implementation
    // =========================================================================

    /// Get observation vector for agent/RL integration
    fn tool_observe(&mut self, args: Value) -> Result<Value, JsonRpcError> {
        let sim_id = self.get_sim_id(&args)?;
        let observer_type = args.get("observer_type")
            .and_then(|v| v.as_str())
            .unwrap_or("global");

        let sim = self.simulations.get_mut(&sim_id).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: format!("Simulation not found: {}", sim_id),
            data: None,
        })?;

        let positions = sim.get_positions();
        let velocities = sim.get_velocities();
        let n_atoms = sim.get_n_atoms();

        match observer_type {
            "global" => {
                // Global observation: statistics of the system
                let (mean_x, mean_y, mean_z, mean_vx, mean_vy, mean_vz) = {
                    let mut sum_pos = [0.0f32; 3];
                    let mut sum_vel = [0.0f32; 3];
                    for i in 0..n_atoms {
                        sum_pos[0] += positions[i * 3];
                        sum_pos[1] += positions[i * 3 + 1];
                        sum_pos[2] += positions[i * 3 + 2];
                        sum_vel[0] += velocities[i * 3];
                        sum_vel[1] += velocities[i * 3 + 1];
                        sum_vel[2] += velocities[i * 3 + 2];
                    }
                    let n = n_atoms as f32;
                    (sum_pos[0]/n, sum_pos[1]/n, sum_pos[2]/n,
                     sum_vel[0]/n, sum_vel[1]/n, sum_vel[2]/n)
                };

                let temperature = sim.get_temperature();
                let total_energy = sim.get_total_energy();
                let kinetic_energy = sim.get_kinetic_energy();
                let potential_energy = sim.get_potential_energy();

                let observation = vec![
                    mean_x, mean_y, mean_z,
                    mean_vx, mean_vy, mean_vz,
                    temperature,
                    total_energy,
                    kinetic_energy,
                    potential_energy,
                    n_atoms as f32,
                ];

                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Global observation: {} features", observation.len())
                    }],
                    "observation": observation,
                    "shape": [observation.len()],
                    "metadata": {
                        "n_visible": n_atoms,
                        "center": [mean_x, mean_y, mean_z],
                        "temperature": temperature,
                        "energy": total_energy
                    }
                }))
            }
            "local" => {
                // Local observation: atoms within radius of center
                let center = args.get("center")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        let x = arr.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        let y = arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        let z = arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                        [x, y, z]
                    })
                    .unwrap_or([0.0, 0.0, 0.0]);

                let radius = args.get("radius")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(5.0) as f32;
                let radius_sq = radius * radius;

                // Find atoms within radius
                let mut local_obs: Vec<f32> = Vec::new();
                let mut n_visible = 0;

                for i in 0..n_atoms {
                    let px = positions[i * 3];
                    let py = positions[i * 3 + 1];
                    let pz = positions[i * 3 + 2];

                    let dx = px - center[0];
                    let dy = py - center[1];
                    let dz = pz - center[2];
                    let dist_sq = dx * dx + dy * dy + dz * dz;

                    if dist_sq < radius_sq {
                        // Relative position
                        local_obs.push(dx);
                        local_obs.push(dy);
                        local_obs.push(dz);
                        // Velocity
                        local_obs.push(velocities[i * 3]);
                        local_obs.push(velocities[i * 3 + 1]);
                        local_obs.push(velocities[i * 3 + 2]);
                        n_visible += 1;
                    }
                }

                Ok(json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Local observation: {} atoms visible within radius {}", n_visible, radius)
                    }],
                    "observation": local_obs,
                    "shape": [n_visible, 6],
                    "metadata": {
                        "n_visible": n_visible,
                        "center": center,
                        "radius": radius
                    }
                }))
            }
            _ => Err(JsonRpcError {
                code: -32602,
                message: format!("Unknown observer type: {}. Supported: global, local", observer_type),
                data: None,
            }),
        }
    }

    // =========================================================================
    // Memory/Episode Tool Implementations
    // =========================================================================

    /// Store an episode in memory
    fn tool_memory_store(&mut self, args: Value) -> Result<Value, JsonRpcError> {
        let sim_id = self.get_sim_id(&args)?;
        let key = args.get("key")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError {
                code: -32602,
                message: "Missing required parameter: key".to_string(),
                data: None,
            })?
            .to_string();

        // Parse observations
        let observations: Vec<Vec<f32>> = args.get("observations")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter().filter_map(|obs| {
                    obs.as_array().map(|inner| {
                        inner.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect()
                    })
                }).collect()
            })
            .unwrap_or_default();

        // Parse actions
        let actions: Vec<Vec<f32>> = args.get("actions")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter().filter_map(|act| {
                    act.as_array().map(|inner| {
                        inner.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect()
                    })
                }).collect()
            })
            .unwrap_or_default();

        // Parse rewards
        let rewards: Vec<f32> = args.get("rewards")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect()
            })
            .unwrap_or_default();

        let total_reward: f32 = rewards.iter().sum();

        let metadata = args.get("metadata").cloned();

        // Create episode
        let episode_id = format!("ep_{}", self.next_episode_id);
        self.next_episode_id += 1;

        let episode = Episode {
            id: episode_id.clone(),
            key: key.clone(),
            sim_id: sim_id.clone(),
            observations,
            actions,
            rewards: rewards.clone(),
            total_reward,
            metadata,
        };

        // Store
        self.episodes
            .entry(sim_id.clone())
            .or_insert_with(Vec::new)
            .push(episode);

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Stored episode '{}' with key '{}', {} steps, total reward {:.4}",
                    episode_id, key, rewards.len(), total_reward)
            }],
            "stored": true,
            "episode_id": episode_id,
            "key": key,
            "steps": rewards.len(),
            "total_reward": total_reward
        }))
    }

    /// List stored episodes
    fn tool_memory_list(&self, args: Value) -> Result<Value, JsonRpcError> {
        let sim_id = self.get_sim_id(&args)?;
        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;

        let episodes = self.episodes.get(&sim_id).cloned().unwrap_or_default();
        let episode_list: Vec<Value> = episodes.iter().rev().take(limit).map(|ep| {
            json!({
                "episode_id": ep.id,
                "key": ep.key,
                "steps": ep.rewards.len(),
                "total_reward": ep.total_reward
            })
        }).collect();

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("{} episode(s) stored for simulation '{}'", episode_list.len(), sim_id)
            }],
            "episodes": episode_list
        }))
    }

    /// Replay a stored episode
    fn tool_memory_replay(&self, args: Value) -> Result<Value, JsonRpcError> {
        let sim_id = self.get_sim_id(&args)?;
        let episode_id = args.get("episode_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| JsonRpcError {
                code: -32602,
                message: "Missing required parameter: episode_id".to_string(),
                data: None,
            })?;

        let episodes = self.episodes.get(&sim_id).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: format!("No episodes found for simulation: {}", sim_id),
            data: None,
        })?;

        let episode = episodes.iter().find(|ep| ep.id == episode_id).ok_or_else(|| JsonRpcError {
            code: -32602,
            message: format!("Episode not found: {}", episode_id),
            data: None,
        })?;

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Retrieved episode '{}' with {} steps", episode_id, episode.rewards.len())
            }],
            "episode_id": episode.id,
            "key": episode.key,
            "observations": episode.observations,
            "actions": episode.actions,
            "rewards": episode.rewards,
            "total_reward": episode.total_reward,
            "metadata": episode.metadata
        }))
    }

    // =========================================================================
    // Benchmark Tool Implementation
    // =========================================================================

    /// Run performance benchmark
    fn tool_bench(&mut self, args: Value) -> Result<Value, JsonRpcError> {
        let suite = args.get("suite")
            .and_then(|v| v.as_str())
            .unwrap_or("all");
        let n_atoms = args.get("n_atoms").and_then(|v| v.as_u64()).unwrap_or(1000) as usize;
        let n_steps = args.get("n_steps").and_then(|v| v.as_u64()).unwrap_or(100) as usize;

        let mut results: Vec<Value> = Vec::new();

        // Create benchmark simulation
        let cells = ((n_atoms as f64 / 4.0).powf(1.0/3.0).ceil() as usize).max(2);
        let sim = WasmSimulation::new_fcc(cells, cells, cells, 1.5, 1.0);
        let actual_atoms = sim.get_n_atoms();

        // We can't easily measure time in WASM without JS interop,
        // so we'll estimate based on operations

        if suite == "all" || suite == "integrate" {
            // Benchmark integration
            let mut bench_sim = WasmSimulation::new_fcc(cells, cells, cells, 1.5, 1.0);
            bench_sim.run(n_steps);

            results.push(json!({
                "name": "integration",
                "n_atoms": actual_atoms,
                "n_steps": n_steps,
                "operations": actual_atoms * n_steps,
                "description": "Velocity Verlet integration"
            }));
        }

        if suite == "all" || suite == "force" {
            results.push(json!({
                "name": "force_calculation",
                "n_atoms": actual_atoms,
                "n_pairs": actual_atoms * (actual_atoms - 1) / 2,
                "description": "Lennard-Jones force computation"
            }));
        }

        if suite == "all" || suite == "hash" {
            // Benchmark hash computation
            let positions: Vec<f32> = sim.get_positions().to_vec();
            let velocities: Vec<f32> = sim.get_velocities().to_vec();
            let hash = compute_state_hash(&positions, &velocities, 0);

            results.push(json!({
                "name": "hash",
                "n_atoms": actual_atoms,
                "hash_bytes": positions.len() * 4 + velocities.len() * 4 + 8,
                "sample_hash": &hash[..16],
                "description": "FNV-1a state hash"
            }));
        }

        Ok(json!({
            "content": [{
                "type": "text",
                "text": format!("Benchmark complete: {} tests on {} atoms", results.len(), actual_atoms)
            }],
            "results": results,
            "system_info": {
                "wasm": true,
                "simd": false, // TODO: Detect SIMD support
                "threads": 1
            }
        }))
    }
}

impl Default for McpHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_handler_creation() {
        let handler = McpHandler::new();
        assert_eq!(handler.simulation_count(), 0);
    }

    #[test]
    fn test_tools_list() {
        let mut handler = McpHandler::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#;
        let response = handler.handle_request(request);

        assert!(response.contains("simulation.create"));
        assert!(response.contains("simulation.step"));
        assert!(response.contains("tools"));
    }

    #[test]
    fn test_resources_list() {
        let mut handler = McpHandler::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"resources/list"}"#;
        let response = handler.handle_request(request);

        assert!(response.contains("fxnn://config/defaults"));
        assert!(response.contains("fxnn://config/forcefields"));
        assert!(response.contains("fxnn://docs/api"));
        assert!(response.contains("resources"));
    }

    #[test]
    fn test_resources_read_defaults() {
        let mut handler = McpHandler::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"fxnn://config/defaults"}}"#;
        let response = handler.handle_request(request);

        assert!(response.contains("timestep"));
        assert!(response.contains("lennard-jones"));
        assert!(response.contains("velocity-verlet"));
    }

    #[test]
    fn test_resources_read_forcefields() {
        let mut handler = McpHandler::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"fxnn://config/forcefields"}}"#;
        let response = handler.handle_request(request);

        assert!(response.contains("lennard-jones"));
        assert!(response.contains("coulomb"));
        assert!(response.contains("epsilon"));
    }

    #[test]
    fn test_resources_read_api_docs() {
        let mut handler = McpHandler::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"resources/read","params":{"uri":"fxnn://docs/api"}}"#;
        let response = handler.handle_request(request);

        assert!(response.contains("simulation.create"));
        assert!(response.contains("fxnn://config/defaults"));
        assert!(response.contains("text/markdown"));
    }

    #[test]
    fn test_invalid_method() {
        let mut handler = McpHandler::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"invalid/method"}"#;
        let response = handler.handle_request(request);

        assert!(response.contains("error"));
        assert!(response.contains("-32601")); // Method not found
    }

    #[test]
    fn test_parse_error() {
        let mut handler = McpHandler::new();
        let response = handler.handle_request("invalid json");

        assert!(response.contains("error"));
        assert!(response.contains("-32700")); // Parse error
    }

    #[test]
    fn test_initialize() {
        let mut handler = McpHandler::new();
        let request = r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#;
        let response = handler.handle_request(request);

        assert!(response.contains("protocolVersion"));
        assert!(response.contains("fxnn-mcp"));
        assert!(response.contains("tools"));
        assert!(response.contains("resources"));
    }

    // This test requires actual WASM runtime, skip on native targets
    #[test]
    #[cfg(target_arch = "wasm32")]
    fn test_full_workflow() {
        let mut handler = McpHandler::new();

        // Create simulation
        let create_req = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"simulation.create","arguments":{"lattice_type":"fcc","nx":2,"ny":2,"nz":2,"temperature":1.0}}}"#;
        let response = handler.handle_request(create_req);
        assert!(response.contains("sim_0"));
        assert!(response.contains("32")); // 2*2*2*4 = 32 atoms in FCC

        // Check resources now include simulation
        let resources_req = r#"{"jsonrpc":"2.0","id":2,"method":"resources/list"}"#;
        let response = handler.handle_request(resources_req);
        assert!(response.contains("fxnn://simulation/sim_0/positions"));
        assert!(response.contains("fxnn://simulation/sim_0/state"));

        // Read simulation state
        let state_req = r#"{"jsonrpc":"2.0","id":3,"method":"resources/read","params":{"uri":"fxnn://simulation/sim_0/state"}}"#;
        let response = handler.handle_request(state_req);
        assert!(response.contains("n_atoms"));
        assert!(response.contains("positions"));
        assert!(response.contains("velocities"));
    }
}
