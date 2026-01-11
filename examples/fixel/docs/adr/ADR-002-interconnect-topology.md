# ADR-002: Interconnect Topology - 2D Mesh with Hierarchical Overlay

## Status

**Accepted**

## Date

2026-01-09

## Context

FIXEL creates a massively parallel compute fabric with 8.3 million (4K) to 33.2 million (8K) cognitum nodes. Efficient communication between these nodes is critical for:

- **Convolution operations**: Require data from neighboring pixels (3x3, 5x5, 7x7 kernels)
- **Pooling operations**: Aggregate values across spatial regions
- **Global operations**: Attention mechanisms, classification outputs, broadcast weights
- **Wave propagation**: Cellular automata, spiking neural networks

Key requirements:
1. **Low latency for local operations**: Convolutions are the dominant workload
2. **Reasonable latency for global operations**: Classification layers, attention
3. **High bandwidth**: 8.3M nodes each needing neighbor access at 100 MHz
4. **Simple routing**: Minimize per-pixel router complexity (transistor budget)
5. **Fault tolerance**: Gracefully handle defective nodes
6. **Scalability**: Work across 4K, 8K, and future 16K resolutions

Physical constraints:
- Maximum wire length within pixel: ~92 microns
- Display dimensions: ~700mm x 400mm (32" 8K)
- Power budget for interconnect: ~10% of pixel power

## Decision

We will implement a **2D mesh topology with a hierarchical overlay network**.

### Architecture

```
LAYER 0: Pixel Mesh (Base Layer)
- Each pixel connects to 4 cardinal neighbors (N, E, S, W)
- 8-bit bidirectional links at pixel clock rate
- 1-cycle latency for neighbor access

LAYER 1: Micro-Tile (4x4 = 16 pixels)
- Internal: Full mesh connectivity within micro-tile
- External: Direct links to 4 neighboring micro-tiles
- 1 micro-tile controller per 16 pixels

LAYER 2: Tile (16x16 = 256 pixels)
- Aggregation point for 16 micro-tiles
- Tile controller handles inter-tile routing
- 1 Gbps links between adjacent tiles

LAYER 3: Super-Tile (64x64 = 4096 pixels)
- Aggregation for 16 tiles
- Handles reduction operations (pooling, attention)
- Direct links to display controller

LAYER 4: Sector (256x256 = 65,536 pixels)
- Coarse-grained coordination
- Global broadcast distribution

LAYER 5: Display Controller
- External interface
- Weight loading, result extraction
```

### Routing Algorithm

XY dimension-ordered routing for deadlock-free operation:
1. Route in X direction until column matches destination
2. Route in Y direction until row matches destination
3. For hierarchical routing: ascend to lowest common ancestor, traverse, descend

## Alternatives Considered

### Alternative 1: 2D Torus

**Pros:**
- Wrap-around links reduce diameter from O(N) to O(N/2)
- Better load balancing for global patterns
- Symmetric from any node

**Cons:**
- Long wrap-around wires (700mm+) at display scale
- Significant power for cross-panel communication
- Complex clocking for long-distance links
- Minimal benefit given hierarchical overlay

**Rejected because:** The wrap-around links are impractical at display dimensions. The hierarchical overlay provides equivalent diameter reduction without the physical routing challenges.

### Alternative 2: Hypercube

**Pros:**
- O(log N) diameter - excellent for global operations
- Rich connectivity enables diverse routing paths
- Well-studied in parallel computing

**Cons:**
- Log2(33.2M) = 25 links per node - infeasible
- Extremely complex router per pixel
- Long-distance links throughout (not just edges)
- Poor match to 2D spatial locality of vision workloads

**Rejected because:** The link count per node is prohibitive within the cognitum transistor budget. Vision workloads exhibit strong 2D locality that hypercube ignores.

### Alternative 3: Fat Tree

**Pros:**
- Full bisection bandwidth
- Natural for reduction operations
- Clear hierarchy matches processing patterns

**Cons:**
- Concentrates traffic at root - bottleneck for broadcast
- Poor for neighbor-to-neighbor patterns (dominant case)
- Asymmetric latency depending on tree position
- Requires dedicated routing hardware at each level

**Rejected because:** Convolution and local operations dominate FIXEL workloads. Fat tree optimizes for the minority case (global operations) while penalizing the majority case (local operations).

### Alternative 4: Shared Bus

**Pros:**
- Simple implementation
- Broadcast is trivial
- Minimal per-pixel overhead

**Cons:**
- O(N) contention - unusable at 8.3M+ nodes
- Bandwidth does not scale
- Single point of failure

**Rejected because:** Completely unscalable to millions of nodes. Would create immediate bottleneck.

## Consequences

### Positive Consequences

1. **Optimal local latency**: 1-cycle access to 4 immediate neighbors enables high-performance convolutions without stalls.

2. **O(sqrt(N)) diameter**: Worst-case latency for 4K is ~6000 pixel hops, but hierarchical routing reduces this to ~100 cycles for most patterns.

3. **Simple per-pixel routing**: XY routing requires minimal state (just coordinates and direction flags), fitting within transistor budget.

4. **Natural locality exploitation**: Vision algorithms inherently exhibit 2D spatial locality; mesh topology directly matches this pattern.

5. **Fault isolation**: Failed pixels affect only immediate neighbors; routing can bypass defects.

6. **Physical layout simplicity**: Mesh maps directly to pixel grid; no long-distance wires within base layer.

### Negative Consequences

1. **Global operation overhead**: Attention mechanisms and global pooling require O(sqrt(N)) hops, adding latency compared to specialized global networks.

2. **Hierarchical complexity**: Multi-level overlay adds design and verification complexity; tile controllers require additional silicon.

3. **Non-uniform latency**: Latency varies based on source-destination distance, complicating performance modeling.

4. **Edge effects**: Pixels at panel edges have fewer neighbors, requiring special handling.

### Performance Characteristics

| Operation | Latency (4K) | Latency (8K) |
|-----------|--------------|--------------|
| Neighbor access | 1 cycle | 1 cycle |
| 3x3 convolution | 9 cycles | 9 cycles |
| Tile reduction (256 px) | 16 cycles | 16 cycles |
| Cross-panel (mesh only) | 6000 cycles | 12000 cycles |
| Cross-panel (hierarchical) | ~100 cycles | ~150 cycles |
| Global broadcast | 1000 cycles | 2000 cycles |

### Aggregate Bandwidth

- Per-pixel: 4 links x 8 bits x 100 MHz = 400 MB/s
- 4K total: 3.32 TB/s aggregate
- 8K total: 13.3 TB/s aggregate

## Related Decisions

- ADR-001 (Cognitum Architecture): Datapath width matches link width
- ADR-003 (Memory Hierarchy): Tile memory shared via interconnect
- ADR-006 (Programming Model): Communication primitives based on mesh

## References

- "On-Chip Networks" - Dally & Towles, Morgan Kaufmann
- "Principles and Practices of Interconnection Networks" - Dally & Towles
- "Network-on-Chip Architectures: A Holistic Design Exploration" - Ogras & Marculescu
