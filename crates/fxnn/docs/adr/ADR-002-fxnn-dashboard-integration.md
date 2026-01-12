# ADR-002: FXNN WASM Dashboard Integration

## Status

**Proposed** - Ready for Implementation

## Date

2026-01-12

## Context

FXNN is a high-performance molecular dynamics simulation library with a complete MCP (Model Context Protocol) implementation. We need to integrate this as a service into the `/dashboard/` web application to provide:

1. **Reality-checking sandbox** - A constrained environment where intelligence operates within physical rules
2. **Interactive simulation visualization** - Real-time WebGL/Three.js rendering
3. **MCP tool interface** - Browser-accessible AI agent integration
4. **Agent observability** - Metrics, witness logs, and state inspection

### Core Principle

> "A small, strict world that runs inside a computer and refuses to let impossible things happen."

- Simulation engine where physics, rules, and limits are **always enforced**
- Agents operate under **partial observability**
- Invalid actions are **resisted or refused**
- Everything is **deterministic, replayable, and measurable**
- Runs **locally, quietly, continuously** even on small chips
- **Remembers** what happened and compares new situations to past ones

### State of the Art (2025-2026)

Based on research, the following technologies represent current best practices:

**WebAssembly + WebGL:**
- [WebGPU achieves 2-5× draw-call throughput](https://faithforgelabs.com/blog_webgpu_wasm.php) over WebGL
- [WASM physics simulations](https://www.researchgate.net/publication/393423079_Enhancing_Browser_Physics_Simulations_WebAssembly_and_Multithreading_Strategies) cut frame times by 50%, reduce memory by 30MB
- Babylon.js 8.0 with WebGPU support for heavy 3D visualization

**MCP UI Integration:**
- [MCP Apps Extension (SEP-1865)](https://blog.modelcontextprotocol.io/posts/2025-11-21-mcp-apps/) standardizes interactive UIs
- [Chrome DevTools MCP Server](https://developer.chrome.com/blog/chrome-devtools-mcp) enables AI assistants to debug web pages
- MCP has 97M+ monthly SDK downloads with first-class support in Claude, ChatGPT, Gemini

## Decision

We will implement the FXNN Dashboard Integration as a new `/dashboard/simulator` section using:

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              DASHBOARD LAYER                                 │
│  React Components (SimulatorPage, McpConsole, VisualizationCanvas, etc.)    │
└─────────────────────────────────┬───────────────────────────────────────────┘
                                  │
┌─────────────────────────────────▼───────────────────────────────────────────┐
│                            WASM-MCP BRIDGE                                   │
│  TypeScript: McpClient, SimulationManager, StateSubscriber                  │
│  Handles JSON-RPC 2.0, tool invocation, resource fetching                   │
└─────────────────────────────────┬───────────────────────────────────────────┘
                                  │ wasm-bindgen
┌─────────────────────────────────▼───────────────────────────────────────────┐
│                            FXNN WASM MODULE                                  │
│  McpHandler: 7 tools, 6 resources                                           │
│  WasmSimulation: Direct simulation API                                      │
│  WasmVisualization: Position/velocity data export                           │
└─────────────────────────────────┬───────────────────────────────────────────┘
                                  │
┌─────────────────────────────────▼───────────────────────────────────────────┐
│                         FXNN CORE ENGINE                                     │
│  Physics: Lennard-Jones, Coulomb, Velocity Verlet                           │
│  Invariants: Energy conservation, momentum, force clamping                  │
│  Witness: Logging, snapshots, deterministic replay                          │
└─────────────────────────────────────────────────────────────────────────────┘
                                  │
┌─────────────────────────────────▼───────────────────────────────────────────┐
│                      OPTIONAL: RUVECTOR EDGE                                 │
│  Memory substrate: Episodes, trajectories, embeddings                       │
│  Similarity search: State comparison, pattern retrieval                     │
│  Edge-net: Distributed simulation coordination                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

### DDD Bounded Contexts

#### 1. SimulationContext (Core Domain)

**Aggregates:**
- `SimulationInstance` - Root aggregate for a running simulation
  - `SimulationId` (value object)
  - `SimulationState` (entity): atoms, box, step, time
  - `ForceFieldConfig` (value object): LJ params, cutoff
  - `IntegratorConfig` (value object): timestep, type

**Domain Events:**
- `SimulationCreated`
- `StepCompleted`
- `StateSnapshotTaken`
- `InvariantViolationDetected`

#### 2. AgencyContext (Supporting Domain)

**Aggregates:**
- `Agent` - Embodied entity with sensors/actuators
  - `AgentId`
  - `SensorArray`: configured sensors
  - `ActuatorArray`: available actions
  - `Policy`: decision-making strategy

**Domain Events:**
- `AgentSpawned`
- `ActionProposed`
- `ActionValidated`
- `ActionRejected`

#### 3. PerceptionContext (Supporting Domain)

**Aggregates:**
- `ObservationPipeline`
  - `BandwidthLimit`
  - `NoiseModel`
  - `AttentionMask`

**Value Objects:**
- `Observation`: agent-specific view of world
- `SensorReading`: typed sensor data

#### 4. GovernanceContext (Supporting Domain)

**Aggregates:**
- `ActionGate`
  - `PermissionSet`
  - `Budget` (energy, action count)
  - `ValidationRules`

**Domain Events:**
- `ActionPermitted`
- `ActionBlocked`
- `BudgetExhausted`

#### 5. WitnessContext (Supporting Domain)

**Aggregates:**
- `WitnessLog`
  - `LogEntry[]`: timestamped events
  - `StateHash`: determinism verification

- `CheckpointManager`
  - `Snapshot[]`: full state captures
  - `RollbackCapability`

#### 6. MemoryContext (Optional - ruvector)

**Aggregates:**
- `EpisodeStore`
  - `Episode`: start → end with metrics
  - `Trajectory`: compressed action sequences
  - `StateEmbedding`: vector representation

**Queries:**
- `FindSimilarStates(embedding, k)`
- `FindSuccessfulTrajectories(goal)`
- `GraphQuery(failure_signature)`

### Module Structure

```
/dashboard/src/
├── pages/dashboard/
│   └── simulator/
│       ├── SimulatorPage.tsx          # Main simulator section
│       ├── SimulatorLayout.tsx        # Layout with panels
│       └── index.ts
├── components/simulator/
│   ├── visualization/
│   │   ├── SimulationCanvas.tsx       # WebGL/Three.js rendering
│   │   ├── AtomRenderer.tsx           # Atom visualization
│   │   ├── ForceVectors.tsx           # Force visualization
│   │   └── BoundingBox.tsx            # Simulation box
│   ├── controls/
│   │   ├── SimulationControls.tsx     # Play/pause/step
│   │   ├── ParameterPanel.tsx         # Timestep, temperature
│   │   ├── ForceFieldSelector.tsx     # LJ, Coulomb configs
│   │   └── SnapshotManager.tsx        # Save/restore states
│   ├── metrics/
│   │   ├── EnergyChart.tsx            # K, V, E over time
│   │   ├── MomentumDisplay.tsx        # Conservation tracking
│   │   ├── InvariantStatus.tsx        # Pass/fail indicators
│   │   └── PerformanceStats.tsx       # Steps/sec, atoms
│   ├── mcp/
│   │   ├── McpConsole.tsx             # JSON-RPC message viewer
│   │   ├── ToolBrowser.tsx            # Available tools list
│   │   ├── ResourceBrowser.tsx        # Resource explorer
│   │   └── RequestBuilder.tsx         # Manual tool invocation
│   ├── witness/
│   │   ├── WitnessLogViewer.tsx       # Event log display
│   │   ├── SnapshotTimeline.tsx       # Visual snapshot history
│   │   └── ReplayControls.tsx         # Deterministic replay
│   └── agents/
│       ├── AgentInspector.tsx         # Agent state viewer
│       ├── SensorDisplay.tsx          # Sensor readings
│       └── PolicyVisualizer.tsx       # Decision visualization
├── lib/wasm/
│   ├── fxnn-loader.ts                 # WASM module loading
│   ├── mcp-client.ts                  # MCP protocol client
│   ├── simulation-manager.ts          # Simulation lifecycle
│   └── state-subscriber.ts            # Reactive state updates
├── hooks/
│   ├── useSimulation.ts               # Simulation state hook
│   ├── useMcpTools.ts                 # Tool invocation hook
│   ├── useMcpResources.ts             # Resource fetching hook
│   └── useWitnessLog.ts               # Log subscription hook
└── types/
    ├── simulation.ts                  # Simulation types
    ├── mcp.ts                         # MCP protocol types
    └── agent.ts                       # Agent types
```

### WASM-MCP TypeScript Bridge

```typescript
// lib/wasm/mcp-client.ts

import init, { McpHandler } from 'fxnn';

interface JsonRpcRequest {
  jsonrpc: '2.0';
  id: number | string;
  method: string;
  params?: unknown;
}

interface JsonRpcResponse {
  jsonrpc: '2.0';
  id: number | string;
  result?: unknown;
  error?: { code: number; message: string; data?: unknown };
}

export class FxnnMcpClient {
  private handler: McpHandler | null = null;
  private requestId = 0;
  private listeners: Map<string, Set<(data: unknown) => void>> = new Map();

  async initialize(): Promise<void> {
    await init();
    this.handler = new McpHandler();
  }

  async listTools(): Promise<McpTool[]> {
    const response = await this.request('tools/list');
    return response.tools;
  }

  async listResources(): Promise<McpResource[]> {
    const response = await this.request('resources/list');
    return response.resources;
  }

  async callTool(name: string, args: unknown): Promise<unknown> {
    return this.request('tools/call', { name, arguments: args });
  }

  async readResource(uri: string): Promise<unknown> {
    return this.request('resources/read', { uri });
  }

  private async request(method: string, params?: unknown): Promise<unknown> {
    if (!this.handler) throw new Error('MCP client not initialized');

    const request: JsonRpcRequest = {
      jsonrpc: '2.0',
      id: ++this.requestId,
      method,
      params
    };

    const responseJson = this.handler.handle_request(JSON.stringify(request));
    const response: JsonRpcResponse = JSON.parse(responseJson);

    if (response.error) {
      throw new McpError(response.error.code, response.error.message);
    }

    return response.result;
  }
}
```

### MCP Tools (Existing)

| Tool | Description | Parameters |
|------|-------------|------------|
| `simulation.create` | Create new simulation | `lattice_type`, `nx/ny/nz`, `spacing`, `temperature` |
| `simulation.step` | Run N steps | `sim_id`, `steps` |
| `simulation.state` | Get current state | `sim_id` |
| `simulation.energy` | Get energy breakdown | `sim_id` |
| `simulation.configure` | Update parameters | `sim_id`, `timestep`, `temperature` |
| `simulation.destroy` | Cleanup | `sim_id` |
| `simulation.list` | List active sims | - |

### MCP Resources (Existing)

| URI | Description | MIME Type |
|-----|-------------|-----------|
| `fxnn://config/defaults` | Default parameters | application/json |
| `fxnn://config/forcefields` | Available force fields | application/json |
| `fxnn://docs/api` | API documentation | text/markdown |
| `fxnn://simulation/{id}/positions` | Atom positions | application/json |
| `fxnn://simulation/{id}/velocities` | Atom velocities | application/json |
| `fxnn://simulation/{id}/state` | Full state snapshot | application/json |

### Optional: ruvector Edge Integration

When `ruvector-edge` feature is enabled:

```typescript
// Additional MCP tools
interface RuvectorTools {
  'memory.store_episode': (episode: Episode) => EpisodeId;
  'memory.search': (query: EmbeddingQuery) => SearchResults;
  'memory.graph_query': (signature: FailureSignature) => GraphPath[];
}

// State embedding schema
interface StateEmbedding {
  global: {
    energy: number;
    temperature: number;
    constraint_violations: number;
  };
  histogram: {
    distance_bins: number[];
    occupancy_bins: number[];
  };
  agent_features?: {
    reward: number;
    entropy: number;
    action_dist: number[];
  };
}
```

### Implementation Milestones

#### Phase 1: Core Integration (Week 1-2)

**Milestone 1.1: WASM Loader & MCP Client**
- [ ] Create `fxnn-loader.ts` with lazy WASM initialization
- [ ] Implement `FxnnMcpClient` with full tool/resource support
- [ ] Add error handling and retry logic
- [ ] **Success Criteria:** Can call all 7 tools and read all 6 resources

**Milestone 1.2: Basic Dashboard Page**
- [ ] Create `SimulatorPage.tsx` with sidebar navigation
- [ ] Add route `/dashboard/simulator` to router
- [ ] Create placeholder components
- [ ] **Success Criteria:** Page renders with navigation working

#### Phase 2: Visualization (Week 2-3)

**Milestone 2.1: WebGL Canvas**
- [ ] Implement `SimulationCanvas.tsx` with Three.js
- [ ] Create `AtomRenderer` with instanced spheres
- [ ] Add camera controls (orbit, zoom, pan)
- [ ] **Success Criteria:** 1000+ atoms render at 60fps

**Milestone 2.2: Real-time Updates**
- [ ] Implement `useSimulation` hook with state subscription
- [ ] Add position streaming from WASM
- [ ] Optimize with requestAnimationFrame
- [ ] **Success Criteria:** Smooth animation during simulation

#### Phase 3: Controls & Metrics (Week 3-4)

**Milestone 3.1: Simulation Controls**
- [ ] Create play/pause/step controls
- [ ] Implement parameter sliders (timestep, temperature)
- [ ] Add lattice configuration form
- [ ] **Success Criteria:** Full control over simulation lifecycle

**Milestone 3.2: Metrics Dashboard**
- [ ] Implement `EnergyChart` with Recharts
- [ ] Add `InvariantStatus` with pass/fail indicators
- [ ] Create `PerformanceStats` display
- [ ] **Success Criteria:** Real-time metrics during simulation

#### Phase 4: MCP UI (Week 4-5)

**Milestone 4.1: MCP Console**
- [ ] Create `McpConsole` with request/response viewer
- [ ] Implement `ToolBrowser` with schema display
- [ ] Add `ResourceBrowser` with live updates
- [ ] **Success Criteria:** Can manually invoke any tool

**Milestone 4.2: Request Builder**
- [ ] Create form-based tool invocation UI
- [ ] Add JSON editor for advanced users
- [ ] Implement request history
- [ ] **Success Criteria:** Non-technical users can use MCP

#### Phase 5: Witness & Replay (Week 5-6)

**Milestone 5.1: Witness Logging**
- [ ] Implement `WitnessLogViewer` component
- [ ] Add filtering by event type
- [ ] Create export functionality
- [ ] **Success Criteria:** Full audit trail visible

**Milestone 5.2: Snapshot & Replay**
- [ ] Implement `SnapshotTimeline` visualization
- [ ] Add save/restore UI
- [ ] Create deterministic replay controls
- [ ] **Success Criteria:** Can replay any simulation identically

#### Phase 6: Optional Features (Week 6+)

**Milestone 6.1: ruvector Integration**
- [ ] Add episode storage UI
- [ ] Implement similarity search interface
- [ ] Create trajectory comparison view
- [ ] **Success Criteria:** Can store and retrieve episodes

**Milestone 6.2: Agent Visualization**
- [ ] Create `AgentInspector` component
- [ ] Add sensor reading display
- [ ] Implement policy visualization
- [ ] **Success Criteria:** Full agent observability

## TDD London School Test Specifications

### Unit Tests (Mocks First)

```typescript
// __tests__/lib/mcp-client.test.ts
describe('FxnnMcpClient', () => {
  let mockHandler: jest.Mocked<McpHandler>;
  let client: FxnnMcpClient;

  beforeEach(() => {
    mockHandler = {
      handle_request: jest.fn(),
      get_server_info: jest.fn(),
      simulation_count: jest.fn()
    };
    client = new FxnnMcpClient(mockHandler);
  });

  describe('listTools', () => {
    it('should return tools from MCP handler', async () => {
      mockHandler.handle_request.mockReturnValue(JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        result: { tools: [{ name: 'simulation.create' }] }
      }));

      const tools = await client.listTools();
      expect(tools).toHaveLength(1);
      expect(tools[0].name).toBe('simulation.create');
    });

    it('should throw on MCP error', async () => {
      mockHandler.handle_request.mockReturnValue(JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        error: { code: -32601, message: 'Method not found' }
      }));

      await expect(client.listTools()).rejects.toThrow('Method not found');
    });
  });

  describe('callTool', () => {
    it('should create simulation with correct params', async () => {
      mockHandler.handle_request.mockReturnValue(JSON.stringify({
        jsonrpc: '2.0',
        id: 1,
        result: { sim_id: 'sim_0', n_atoms: 256 }
      }));

      const result = await client.callTool('simulation.create', {
        lattice_type: 'fcc',
        nx: 4, ny: 4, nz: 4
      });

      expect(result.sim_id).toBe('sim_0');
      expect(mockHandler.handle_request).toHaveBeenCalledWith(
        expect.stringContaining('simulation.create')
      );
    });
  });
});

// __tests__/components/SimulationCanvas.test.tsx
describe('SimulationCanvas', () => {
  let mockSimulation: jest.Mocked<SimulationManager>;

  beforeEach(() => {
    mockSimulation = {
      getPositions: jest.fn().mockReturnValue(new Float32Array(300)),
      getAtomCount: jest.fn().mockReturnValue(100),
      subscribe: jest.fn()
    };
  });

  it('should render atom instances', () => {
    render(<SimulationCanvas simulation={mockSimulation} />);
    expect(mockSimulation.getPositions).toHaveBeenCalled();
  });

  it('should update on position change', () => {
    const { rerender } = render(<SimulationCanvas simulation={mockSimulation} />);

    mockSimulation.getPositions.mockReturnValue(new Float32Array(600));
    mockSimulation.getAtomCount.mockReturnValue(200);

    rerender(<SimulationCanvas simulation={mockSimulation} />);
    expect(mockSimulation.getAtomCount).toHaveBeenCalledTimes(2);
  });
});
```

### Integration Tests

```typescript
// __tests__/integration/simulator-workflow.test.ts
describe('Simulator Workflow', () => {
  it('should complete full simulation cycle', async () => {
    // 1. Initialize WASM
    const client = new FxnnMcpClient();
    await client.initialize();

    // 2. Create simulation
    const createResult = await client.callTool('simulation.create', {
      lattice_type: 'fcc',
      nx: 2, ny: 2, nz: 2,
      temperature: 1.0
    });
    expect(createResult.sim_id).toBeDefined();

    // 3. Run steps
    const stepResult = await client.callTool('simulation.step', {
      sim_id: createResult.sim_id,
      steps: 100
    });
    expect(stepResult.current_step).toBe(100);

    // 4. Get state
    const state = await client.readResource(
      `fxnn://simulation/${createResult.sim_id}/state`
    );
    expect(state.contents[0]).toBeDefined();

    // 5. Cleanup
    await client.callTool('simulation.destroy', {
      sim_id: createResult.sim_id
    });
  });
});
```

## Consequences

### Positive

1. **Unified simulation interface** - Browser and server use same MCP protocol
2. **AI agent ready** - Claude/GPT can interact via standard MCP
3. **Observable** - Full metrics, logging, and replay capability
4. **Deterministic** - Reproducible results for debugging/testing
5. **Progressive enhancement** - Optional ruvector adds memory/learning

### Negative

1. **WASM bundle size** - ~2-5MB initial download
2. **WebGL requirement** - No fallback for non-WebGL browsers
3. **Complex state management** - Multiple layers of state synchronization
4. **Learning curve** - DDD patterns require team familiarity

### Mitigation

1. **Bundle size** - Lazy loading, CDN caching, WASM streaming
2. **WebGL** - Detect capability, show message for unsupported browsers
3. **State management** - Use React Query for server state, Zustand for UI
4. **Learning curve** - Documentation, pair programming, code reviews

## References

- [ADR-001: Five-Layer Reality Stack](ADR-001-five-layer-reality-stack.md)
- [ASR-003: Cognitum v0 Simulation Specification](/workspaces/newport/plans/atomic/ASR-003-Agentic-Simulator.md)
- [MCP Apps Extension](https://blog.modelcontextprotocol.io/posts/2025-11-21-mcp-apps/)
- [WebAssembly Physics Simulations](https://www.researchgate.net/publication/393423079_Enhancing_Browser_Physics_Simulations_WebAssembly_and_Multithreading_Strategies)
- [WebGPU & WASM Deep Dive](https://faithforgelabs.com/blog_webgpu_wasm.php)

## Changelog

### 2026-01-12 - Initial Draft

- Created ADR-002 for FXNN Dashboard Integration
- Defined 6 DDD bounded contexts
- Specified module structure
- Outlined 6-phase implementation with milestones
- Added TDD London School test specifications
