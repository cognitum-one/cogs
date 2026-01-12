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
    next_sim_id: u32,
}

#[wasm_bindgen]
impl McpHandler {
    /// Create a new MCP handler instance.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            simulations: HashMap::new(),
            next_sim_id: 0,
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
            "simulation.create" => self.tool_simulation_create(call_params.arguments),
            "simulation.step" => self.tool_simulation_step(call_params.arguments),
            "simulation.state" => self.tool_simulation_state(call_params.arguments),
            "simulation.energy" => self.tool_simulation_energy(call_params.arguments),
            "simulation.configure" => self.tool_simulation_configure(call_params.arguments),
            "simulation.destroy" => self.tool_simulation_destroy(call_params.arguments),
            "simulation.list" => self.tool_simulation_list(),
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
