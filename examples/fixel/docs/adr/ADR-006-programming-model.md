# ADR-006: Programming Model - SPMD Kernel Execution with Dataflow Option

## Status

**Accepted**

## Date

2026-01-09

## Context

FIXEL presents unique programming challenges:
- **Massive parallelism**: 8.3M to 33.2M independent compute units
- **Limited per-unit memory**: 512B-2KB per pixel
- **Spatial structure**: 2D grid with nearest-neighbor connectivity
- **Heterogeneous workloads**: Neural networks, image processing, simulations

Target developer populations:
1. **GPU programmers**: Familiar with CUDA/OpenCL, expect SIMT model
2. **ML researchers**: Use PyTorch/TensorFlow, expect layer abstractions
3. **Image processing experts**: Expect filter/kernel primitives
4. **Simulation scientists**: Expect cellular automata / PDE abstractions

Key requirements:
1. **Ease of adoption**: Leverage existing GPU/parallel programming skills
2. **Performance**: Map efficiently to hardware capabilities
3. **Portability**: Work across density tiers with graceful scaling
4. **Debugging**: Support debugging millions of parallel threads
5. **Composability**: Build complex applications from simple primitives

Execution constraints:
- No global synchronization within single cycle
- Limited instruction memory per pixel
- Deterministic execution required for reproducibility

## Decision

We will implement **SPMD (Single Program, Multiple Data) kernel execution** as the primary model, with **optional dataflow execution** for streaming workloads.

### SPMD Model (Primary)

```python
@fixel_kernel
def sobel_edge_detect(pixel: Pixel) -> None:
    """
    Every pixel executes this function simultaneously.
    Each pixel operates on its own data (coordinates, neighbors).
    """
    # Load neighbors (1 cycle each via mesh)
    n = pixel.north.value
    s = pixel.south.value
    e = pixel.east.value
    w = pixel.west.value
    ne = pixel.north.east.value
    nw = pixel.north.west.value
    se = pixel.south.east.value
    sw = pixel.south.west.value

    # Sobel operators
    gx = (ne + 2*e + se) - (nw + 2*w + sw)
    gy = (nw + 2*n + ne) - (sw + 2*s + se)

    # Magnitude (approximate L1 norm)
    magnitude = abs(gx) + abs(gy)

    # Output
    pixel.output = min(255, magnitude)

# Launch on entire display
fixel.execute(sobel_edge_detect)
```

### Kernel Execution Semantics

```
EXECUTION MODEL:
1. All pixels load the same kernel code
2. All pixels begin execution at cycle 0
3. Each pixel has unique (x, y) coordinates
4. Neighbor access is synchronous within cycle
5. Tile barriers synchronize within tile boundaries
6. Global barriers require wave propagation

MEMORY MODEL:
- Registers: Private per pixel
- SRAM: Private per pixel
- Tile memory: Shared within tile (explicit sync)
- Neighbor data: 1-cycle access via mesh

SYNCHRONIZATION:
- Implicit: All neighbors complete cycle N before cycle N+1
- Explicit: tile_barrier() for tile-wide sync
- Global: wave_sync() for display-wide (expensive)
```

### Dataflow Model (Optional)

```python
@fixel_dataflow
def neural_layer(pixel: PixelDataflow) -> None:
    """
    Dataflow execution: pixel computes when inputs arrive.
    No global synchronization; data-driven activation.
    """
    # Block until input available
    input_vec = pixel.await_input()  # From previous layer

    # Load local weights
    weights = pixel.sram[0:64]

    # Compute dot product
    output = 0
    for i in range(64):
        output += input_vec[i] * weights[i]

    output = relu(output)

    # Send to next layer (routed by layer mapping)
    pixel.emit_output(output)
```

### Programming Layers

```
LAYER 4: ML Frameworks (PyTorch/TensorFlow)
    └── model.compile(target="fixel")

LAYER 3: Domain Libraries
    ├── fixel.nn.conv2d(kernel=3, filters=64)
    ├── fixel.image.sobel()
    └── fixel.sim.game_of_life()

LAYER 2: Kernel Language (FixelC / FixelPy)
    └── @fixel_kernel decorated functions

LAYER 1: Assembly (Cognitum-8 ISA)
    └── LOAD, STORE, MAC, LNBR, SNBR, etc.

LAYER 0: Hardware
    └── Pixel cognitum execution
```

### Compiler Pipeline

```
Source (Python/C++)
       │
       ▼
┌──────────────────┐
│   FIXEL Frontend │  Parse, type-check, semantic analysis
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│   IR Generation  │  Intermediate representation
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│   Optimization   │  Loop tiling, neighbor prefetch
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│   Tile Mapping   │  Map operations to pixel grid
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│   Code Gen       │  Cognitum-8 binary
└────────┬─────────┘
         │
         ▼
    Binary Image
```

## Alternatives Considered

### Alternative 1: Pure SIMD (Single Instruction, Multiple Data)

**Pros:**
- Simplest hardware (no per-pixel program counter)
- Guaranteed lock-step execution
- Familiar from vector processors

**Cons:**
- No data-dependent branching per pixel
- Inactive pixels waste cycles during divergent paths
- Poor for irregular workloads (object detection, sparse data)
- Cannot handle varying compute per pixel

**Rejected because:** Neural networks and image processing frequently have data-dependent operations (ReLU, thresholding, conditional updates). Pure SIMD would require predication, wasting significant compute.

### Alternative 2: Pure Dataflow

**Pros:**
- Natural for streaming/pipeline workloads
- No explicit synchronization needed
- Energy-efficient (compute only when data available)
- Excellent for spiking neural networks

**Cons:**
- Difficult to reason about performance
- Non-deterministic execution order
- Debugging extremely challenging
- Poor for globally-synchronized operations

**Rejected because:** Many image processing operations (convolutions, reductions) require deterministic neighbor access patterns. Pure dataflow makes these difficult to express and debug.

### Alternative 3: Task Graphs

**Pros:**
- Rich dependency expression
- Natural for complex pipelines
- Enables sophisticated scheduling

**Cons:**
- High per-task overhead for fine-grained operations
- 33.2M tasks per frame is impractical
- Complex runtime scheduler required
- Poor match to pixel-local operations

**Rejected because:** The granularity mismatch is severe. Task graph overhead would dominate actual compute for pixel-local operations.

### Alternative 4: Shared Memory Threading

**Pros:**
- Familiar from CPU programming
- Flexible work distribution
- Dynamic load balancing

**Cons:**
- Requires complex synchronization primitives
- Race conditions in 33.2M threads are undebuggable
- Memory consistency model complexity
- Lock contention at scale

**Rejected because:** Traditional threading models cannot scale to millions of threads. The synchronization overhead and debugging complexity are prohibitive.

## Consequences

### Positive Consequences

1. **GPU programmer familiarity**: SPMD model mirrors CUDA/OpenCL kernels; existing skills transfer directly.

2. **Deterministic execution**: Lock-step neighbor access enables reproducible results and simplified debugging.

3. **Efficient mapping**: Kernels map directly to pixel grid without complex scheduling.

4. **Flexible branching**: Each pixel can take different paths (unlike pure SIMD) while maintaining overall structure.

5. **Composable abstractions**: Layer 3 libraries hide complexity while enabling Layer 2 customization when needed.

### Negative Consequences

1. **Global synchronization cost**: Operations requiring display-wide consistency (global pooling, attention) incur O(sqrt(N)) latency.

2. **Divergence overhead**: Pixels taking different branches cannot share instruction fetch, increasing power.

3. **Learning curve for dataflow**: The optional dataflow model requires different mental model from SPMD.

4. **Limited dynamic parallelism**: Cannot spawn new work from within kernels (fixed parallelism).

### Code Example: Multi-Layer CNN

```python
# Layer definitions
@fixel_kernel
def conv_layer_1(pixel: Pixel) -> None:
    # 3x3 convolution, 8 input channels, 16 output channels
    inputs = gather_neighbors_3x3(pixel, channels=8)
    weights = pixel.sram[0:144]  # 3*3*8 = 72 * 2 bytes

    for out_ch in range(16):
        acc = 0
        for i in range(72):
            acc += inputs[i] * weights[out_ch * 72 + i]
        pixel.features[out_ch] = relu(acc >> 8)

@fixel_kernel
def pooling_2x2(pixel: Pixel) -> None:
    # Max pooling (only 1 in 4 pixels active)
    if pixel.x % 2 == 0 and pixel.y % 2 == 0:
        p00 = pixel.features
        p01 = pixel.east.features
        p10 = pixel.south.features
        p11 = pixel.south.east.features

        for ch in range(16):
            pixel.pooled[ch] = max(p00[ch], p01[ch], p10[ch], p11[ch])

# Execute pipeline
fixel.execute(conv_layer_1)
fixel.tile_barrier()
fixel.execute(pooling_2x2)
```

### Performance Characteristics

| Operation | Cycles | Notes |
|-----------|--------|-------|
| Neighbor load | 1 | Via mesh |
| MAC operation | 1 | 8x8 -> 16 |
| Activation (ReLU) | 1 | Direct compare |
| Activation (sigmoid) | 3 | LUT lookup |
| Tile barrier | 16 | Wave across 16x16 |
| Global barrier | 6000 | Wave across 4K |
| 3x3 convolution | 20-30 | Typical implementation |

## Related Decisions

- ADR-001 (Cognitum Architecture): ISA designed for SPMD kernels
- ADR-002 (Interconnect Topology): Mesh enables efficient neighbor access
- ADR-007 (Neural Substrate): Spiking mode uses dataflow variant

## References

- "CUDA Programming Guide" - NVIDIA
- "Data-Parallel Haskell" - Chakravarty et al.
- "Halide: A Language and Compiler for Optimizing Parallelism" - Ragan-Kelley et al.
