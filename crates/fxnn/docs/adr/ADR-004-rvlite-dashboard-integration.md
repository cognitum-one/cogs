# ADR-004: RvLite Integration for Dashboard Memory & Vector Search

## Status

**Accepted**

## Date

2026-01-12

## Context

The Cognitum Dashboard needs persistent storage and intelligent search capabilities for:
- **Episode Memory**: Store RL training episodes for replay and analysis
- **Snapshot Storage**: Persist simulation snapshots across sessions (IndexedDB)
- **Semantic Search**: Find similar simulation configurations and results
- **Graph Relationships**: Track relationships between simulations, snapshots, and episodes

The dashboard currently uses in-memory storage for MCP tool data, which is lost on page reload.

### Available Storage Options

| Option | Persistence | Search | Graph | WASM Compatible |
|--------|-------------|--------|-------|-----------------|
| localStorage | Yes | No | No | Yes |
| IndexedDB | Yes | No | No | Yes |
| SQLite WASM | Yes | No | No | Yes |
| **RvLite** | Yes (IndexedDB) | **Vector** | **Cypher** | **Yes** |

RvLite provides a unified solution combining:
- Vector similarity search for semantic queries
- SQL queries with vector distance operations
- Cypher property graph queries for relationships
- IndexedDB persistence for browser environments

## Decision

We will integrate RvLite from `/workspaces/newport/ruvector-upstream/npm/packages/rvlite/` into the dashboard for:

1. **Simulation Snapshot Persistence** (IndexedDB)
2. **Episode Memory with Vector Search**
3. **Configuration/Pattern Semantic Search**
4. **Simulation Relationship Graphs**

### Integration Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Dashboard                                    │
├───────────────────────┬─────────────────────────────────────────────┤
│   FXNN WASM Module    │         RvLite WASM Module                  │
│   ┌─────────────────┐ │  ┌─────────────────┐  ┌─────────────────┐   │
│   │ MCP Handler     │ │  │ Vector Store    │  │ Graph Store     │   │
│   │ - snapshots     │←┼──│ - embeddings    │  │ - simulations   │   │
│   │ - episodes      │ │  │ - similarity    │  │ - relationships │   │
│   │ - witness       │ │  │ - clustering    │  │ - lineage       │   │
│   └─────────────────┘ │  └─────────────────┘  └─────────────────┘   │
│          │            │           │                    │            │
│          ▼            │           ▼                    ▼            │
│   ┌─────────────────┐ │  ┌─────────────────────────────────────┐   │
│   │ In-Memory State │ │  │         IndexedDB Persistence       │   │
│   └─────────────────┘ │  └─────────────────────────────────────┘   │
└───────────────────────┴─────────────────────────────────────────────┘
```

### New Dashboard Features

#### 1. Persistent Snapshot Storage

```typescript
import { RvLite, SemanticMemory } from 'rvlite';

// Initialize RvLite with vector dimensions for state embeddings
const db = new RvLite({ dimensions: 64 });  // State embedding size
await db.init();

// Store snapshot with embedding
async function storeSnapshot(snapshot: Snapshot) {
  // Create embedding from positions/velocities statistics
  const embedding = computeStateEmbedding(snapshot);

  await db.insertWithId(snapshot.id, embedding, {
    sim_id: snapshot.sim_id,
    step: snapshot.step,
    hash: snapshot.hash,
    timestamp: snapshot.timestamp,
    positions: snapshot.positions,
    velocities: snapshot.velocities,
  });

  // Store graph relationship
  await db.cypher(`
    MATCH (s:Simulation {id: "${snapshot.sim_id}"})
    CREATE (snap:Snapshot {id: "${snapshot.id}", step: ${snapshot.step}, hash: "${snapshot.hash}"})
    CREATE (s)-[:HAS_SNAPSHOT]->(snap)
  `);
}

// Find similar snapshots
async function findSimilarSnapshots(snapshot: Snapshot, k: number = 5) {
  const embedding = computeStateEmbedding(snapshot);
  return db.search(embedding, k);
}
```

#### 2. Episode Memory with Semantic Search

```typescript
// Store episode with semantic embedding
async function storeEpisode(episode: Episode) {
  // Compute trajectory embedding (mean/std of observations)
  const embedding = computeTrajectoryEmbedding(episode);

  await db.insertWithId(episode.id, embedding, {
    key: episode.key,
    sim_id: episode.sim_id,
    total_reward: episode.total_reward,
    steps: episode.rewards.length,
    // Store compressed trajectory data
    trajectory: compressTrajectory(episode),
  });
}

// Find similar episodes for transfer learning
async function findSimilarEpisodes(query: number[], k: number = 10) {
  return db.search(query, k);
}
```

#### 3. Configuration Pattern Search

```typescript
// Store successful configurations
async function storeConfigPattern(config: SimulationConfig, metrics: Metrics) {
  const embedding = configToEmbedding(config);

  await db.insertWithId(`config_${Date.now()}`, embedding, {
    config,
    metrics,
    success: metrics.energy_conservation < 0.01,
  });
}

// Find configurations similar to current
async function suggestConfigurations(currentConfig: SimulationConfig) {
  const embedding = configToEmbedding(currentConfig);
  const similar = await db.search(embedding, 5);

  // Filter by success rate
  return similar.filter(r => r.metadata?.success === true);
}
```

#### 4. Simulation Lineage Graph

```typescript
// Track simulation lineage
async function trackLineage(simId: string, parentId?: string, action?: string) {
  // Create simulation node
  await db.cypher(`
    CREATE (s:Simulation {
      id: "${simId}",
      created: ${Date.now()},
      ${parentId ? `parent: "${parentId}",` : ''}
      action: "${action || 'create'}"
    })
  `);

  // Link to parent if exists
  if (parentId) {
    await db.cypher(`
      MATCH (parent:Simulation {id: "${parentId}"})
      MATCH (child:Simulation {id: "${simId}"})
      CREATE (parent)-[:DERIVED {action: "${action}"}]->(child)
    `);
  }
}

// Get simulation lineage tree
async function getLineageTree(simId: string) {
  return db.cypher(`
    MATCH path = (root:Simulation)-[:DERIVED*0..]->(s:Simulation {id: "${simId}"})
    RETURN path
  `);
}
```

### State Embedding Computation

For semantic search over simulation states, we compute embeddings from physical properties:

```typescript
function computeStateEmbedding(snapshot: Snapshot): number[] {
  const positions = snapshot.positions;
  const velocities = snapshot.velocities;
  const n = positions.length / 3;

  // Statistical features (64 dimensions)
  const embedding: number[] = [];

  // Position statistics (18 dims)
  const posStats = computeAxisStats(positions, n);
  embedding.push(...posStats);

  // Velocity statistics (18 dims)
  const velStats = computeAxisStats(velocities, n);
  embedding.push(...velStats);

  // Energy features (4 dims)
  embedding.push(snapshot.kinetic_energy || 0);
  embedding.push(snapshot.potential_energy || 0);
  embedding.push(snapshot.temperature || 0);
  embedding.push(n);

  // Spatial distribution (24 dims)
  const spatial = computeSpatialHistogram(positions, n);
  embedding.push(...spatial);

  return normalizeEmbedding(embedding);
}

function computeAxisStats(data: number[], n: number): number[] {
  // For each axis (x, y, z): mean, std, min, max, skew, kurtosis
  const stats: number[] = [];
  for (let axis = 0; axis < 3; axis++) {
    const values = [];
    for (let i = 0; i < n; i++) {
      values.push(data[i * 3 + axis]);
    }
    stats.push(
      mean(values),
      std(values),
      Math.min(...values),
      Math.max(...values),
      skewness(values),
      kurtosis(values)
    );
  }
  return stats;
}
```

### Dashboard Service Integration

```typescript
// src/lib/rvlite/SimulationMemory.ts

import { RvLite, SemanticMemory } from '@/lib/rvlite';

export class SimulationMemoryService {
  private db: RvLite;
  private memory: SemanticMemory;

  async initialize() {
    this.db = new RvLite({ dimensions: 64 });
    await this.db.init();

    // Try to load existing data
    try {
      await RvLite.load();
    } catch {
      // Fresh database
    }
  }

  async saveSnapshot(snapshot: Snapshot) {
    // ... implementation
  }

  async loadSnapshots(simId: string) {
    return this.db.sql(`
      SELECT * FROM vectors
      WHERE metadata->>'sim_id' = '${simId}'
      ORDER BY metadata->>'step' DESC
    `);
  }

  async persist() {
    await this.db.save();  // Save to IndexedDB
  }
}
```

### React Hook Integration

```typescript
// src/hooks/useSimulationMemory.ts

import { useEffect, useState, useCallback } from 'react';
import { SimulationMemoryService } from '@/lib/rvlite/SimulationMemory';

export function useSimulationMemory() {
  const [service] = useState(() => new SimulationMemoryService());
  const [initialized, setInitialized] = useState(false);

  useEffect(() => {
    service.initialize().then(() => setInitialized(true));

    // Persist on page unload
    const handleUnload = () => service.persist();
    window.addEventListener('beforeunload', handleUnload);
    return () => window.removeEventListener('beforeunload', handleUnload);
  }, []);

  const saveSnapshot = useCallback(async (snapshot: Snapshot) => {
    if (!initialized) return;
    await service.saveSnapshot(snapshot);
  }, [initialized, service]);

  const findSimilar = useCallback(async (snapshot: Snapshot, k: number = 5) => {
    if (!initialized) return [];
    return service.findSimilar(snapshot, k);
  }, [initialized, service]);

  return { initialized, saveSnapshot, findSimilar };
}
```

## Implementation Plan

### Phase 1: Core Integration (Priority: Critical)
1. Add rvlite as dashboard dependency
2. Create `SimulationMemoryService` wrapper
3. Integrate with existing MCP hook (`useWasmMcp`)
4. Add IndexedDB persistence on snapshot/episode creation

### Phase 2: Search Features (Priority: High)
1. Implement state embedding computation
2. Add "Find Similar" UI for snapshots
3. Add configuration pattern suggestions
4. Episode similarity search for RL

### Phase 3: Graph Features (Priority: Medium)
1. Implement simulation lineage tracking
2. Add lineage visualization component
3. Add relationship queries to MCP console

### Phase 4: Advanced Features (Priority: Low)
1. SPARQL integration for RDF export
2. Cross-session pattern learning
3. Collaborative memory sharing

## File Structure

```
dashboard/src/lib/rvlite/
├── index.ts                    # Re-exports
├── SimulationMemoryService.ts  # Main service class
├── embeddings.ts               # State embedding computation
├── schemas.ts                  # Cypher graph schemas
└── hooks/
    ├── useSimulationMemory.ts  # React hook
    └── useEpisodeSearch.ts     # Episode search hook
```

## Package Installation

```bash
# Option 1: Link local package
cd dashboard
npm link ../ruvector-upstream/npm/packages/rvlite

# Option 2: Relative import in package.json
{
  "dependencies": {
    "rvlite": "file:../ruvector-upstream/npm/packages/rvlite"
  }
}

# Option 3: Build and copy WASM
cd ../ruvector-upstream/npm/packages/rvlite
npm run build
cp -r dist ../../../dashboard/src/lib/rvlite/dist
```

## Consequences

### Positive
- Persistent simulation data across sessions
- Semantic search for finding similar states
- Graph queries for lineage tracking
- Unified storage solution (vector + SQL + graph)
- WASM performance for large datasets

### Negative
- Additional WASM bundle size (~200KB compressed)
- Learning curve for Cypher/SPARQL queries
- IndexedDB storage limits in browsers

### Mitigations
- Lazy load rvlite WASM only when needed
- Provide SQL as default query interface
- Implement data pruning for old snapshots
- Add storage usage monitoring to UI

## References

- [RvLite Source](../../ruvector-upstream/npm/packages/rvlite/)
- [ADR-001: Five-Layer Reality Stack](./ADR-001-five-layer-reality-stack.md)
- [ADR-003: MCP Protocol Enhancements](./ADR-003-mcp-protocol-enhancements.md)
- [IndexedDB API](https://developer.mozilla.org/en-US/docs/Web/API/IndexedDB_API)
