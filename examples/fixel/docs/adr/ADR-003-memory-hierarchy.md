# ADR-003: Memory Hierarchy - Per-Pixel SRAM with Shared Tile Memory

## Status

**Accepted**

## Date

2026-01-09

## Context

The FIXEL architecture requires efficient storage for:
- **Neural network weights**: Convolutional filters, fully-connected layer parameters
- **Feature maps**: Intermediate activations between layers
- **Pixel state**: Current values, gradients, history for temporal operations
- **Program code**: Instructions for cognitum execution

Memory constraints at 2nm:
- 6T SRAM cell size: ~0.021 um^2
- Per-pixel area budget: ~0.01 mm^2 (10,000 um^2)
- Available for SRAM: ~65% of transistor budget = 1.5M transistors = ~250,000 6T cells = 250 Kbits = 31.25 KB theoretical max
- Practical limit with decode logic: 512B - 1KB per pixel

Workload memory requirements:
- 3x3 convolution: 9 weights + 9 inputs + 1 output = 19 bytes minimum
- ResNet-50: 25M parameters = 25MB total (INT8)
- MobileNet-V2: 3.4M parameters = 3.4MB total (INT8)
- Single layer activations (per pixel): 64-256 bytes typical

Key challenges:
1. Limited per-pixel capacity vs. model size
2. Weight distribution across millions of pixels
3. Bandwidth for weight updates/streaming
4. Feature map storage during multi-layer inference

## Decision

We will implement a **three-level memory hierarchy with per-pixel SRAM and shared tile memory**.

### Architecture

```
LEVEL 1: Per-Pixel Registers (64 bytes)
- 16 x 32-bit general-purpose registers
- 1-cycle access latency
- Used for: Active computation, neighbor data staging

LEVEL 2: Per-Pixel SRAM (512 bytes)
- Local weights, feature map history
- 1-cycle access latency
- 6T SRAM cells with single-port access

LEVEL 3: Shared Tile Memory (64 KB per 16x16 tile)
- 256 bytes per pixel contribution
- Aggregated features, shared weights
- 8-16 cycle access latency (via tile controller)

LEVEL 4 (Optional): External DRAM
- Connected via display controller
- Model weights, frame buffers
- 1000+ cycle access latency
```

### Capacity Summary

| Level | Per-Pixel | Per-Tile (256 px) | 4K Total | 8K Total |
|-------|-----------|-------------------|----------|----------|
| L1 Registers | 64 B | 16 KB | 532 MB | 2.1 GB |
| L2 SRAM | 512 B | 128 KB | 4.25 GB | 17 GB |
| L3 Tile | 256 B (share) | 64 KB | 2.1 GB | 8.4 GB |
| **On-chip Total** | 576 B | 144 KB | 4.78 GB | 19.1 GB |

### Weight Distribution Strategy

Weights are distributed using one of three modes:

**Mode A - Replicated (simple CNN):**
- Same 512B weights stored in every pixel
- Total unique weights: 512 bytes
- Use case: Simple filters, edge detection

**Mode B - Distributed Unique (large models):**
- Each pixel stores different weights
- Total unique weights: 512B x 8.3M = 4.25GB (4K)
- Use case: Per-pixel experts, self-organizing maps

**Mode C - Streaming (largest models):**
- Controller broadcasts weights in waves
- Pixels cache most recently used weights
- Effective model size: Unlimited
- Bandwidth requirement: 256 GB/s for real-time

## Alternatives Considered

### Alternative 1: All Shared Memory (No Per-Pixel SRAM)

**Pros:**
- Maximum effective capacity (pool all memory)
- Flexible allocation based on workload
- Simplified per-pixel design

**Cons:**
- Every memory access requires network traversal
- Contention at shared memory controllers
- High latency for local operations (10+ cycles vs 1)
- Power overhead for constant communication

**Rejected because:** Convolution workloads require repeated neighbor access. Remote memory access would add 10x latency penalty for the dominant operation pattern.

### Alternative 2: All Distributed Memory (No Sharing)

**Pros:**
- Zero contention for local access
- Maximum parallelism
- Predictable latency

**Cons:**
- Cannot store layers larger than 512B per pixel
- No mechanism for feature map passing between layers
- Weights fragmented with no recombination

**Rejected because:** Modern neural networks require feature maps that span multiple pixels. Purely distributed memory cannot support multi-layer inference.

### Alternative 3: DRAM Backing Store (No Tile SRAM)

**Pros:**
- Massive effective capacity (GB to TB)
- Standard memory technology
- Lower cost per bit

**Cons:**
- 100-1000x higher access latency (ns to us)
- Much higher power per access (10x)
- Requires memory controller per display
- Bandwidth limited by display interface

**Rejected because:** Real-time inference requires sub-millisecond latency. DRAM access times would dominate compute time, eliminating the FIXEL latency advantage.

### Alternative 4: eDRAM Instead of SRAM

**Pros:**
- 3-4x density improvement (more bytes per pixel)
- Lower leakage power

**Cons:**
- Requires refresh cycles (complexity)
- Slower access time (2-3x)
- More complex manufacturing at 2nm
- Higher variation in retention time

**Rejected because:** The access latency penalty impacts convolution performance. Manufacturing complexity at 2nm is already challenging without adding eDRAM process steps.

## Consequences

### Positive Consequences

1. **High bandwidth for local operations**: Per-pixel SRAM enables 400 GB/s aggregate bandwidth for neighbor data access (4K at 100 MHz).

2. **Efficient convolutions**: All data for 3x3 kernel fits in registers; 5x5 and 7x7 fit in L2 SRAM with room for weights.

3. **Scalable model support**: Streaming mode enables models of arbitrary size, with tile memory providing cache-like behavior.

4. **Low latency inference**: 1-cycle local access eliminates memory stalls for typical operations.

5. **Power efficiency**: Near-memory compute eliminates long-distance data movement - the dominant power consumer in traditional architectures.

### Negative Consequences

1. **Limited capacity per pixel**: 512B restricts complexity of per-pixel operations; large models require streaming.

2. **Weight streaming bandwidth**: Real-time streaming of large models requires 256+ GB/s from controller, stressing display interface.

3. **Feature map constraints**: Multi-layer networks with large intermediate activations must carefully manage tile memory allocation.

4. **No caching flexibility**: Fixed 512B per pixel cannot be dynamically reallocated; underutilized for simple workloads.

### Memory Access Energy

| Access Type | Energy (pJ) | Relative |
|-------------|-------------|----------|
| Register | 0.01 | 1x |
| L2 SRAM | 0.10 | 10x |
| L3 Tile | 1.0 | 100x |
| External DRAM | 100 | 10,000x |

### Mitigation Strategies

1. **Weight compression**: 4:1 compression of neural network weights effectively quadruples per-pixel model capacity.

2. **Streaming pipeline**: Double-buffering in tile memory hides streaming latency during computation.

3. **Quantization-aware training**: 4-bit weights reduce storage by 2x with minimal accuracy loss for many models.

4. **Layer fusion**: Combine multiple operations to reduce intermediate feature map storage.

## Related Decisions

- ADR-001 (Cognitum Architecture): 8-bit datapath matches memory width
- ADR-002 (Interconnect Topology): Tile structure provides memory hierarchy boundaries
- ADR-005 (Power Management): Memory gating controls significant power fraction

## References

- "Memory Systems: Cache, DRAM, Disk" - Jacob, Ng, Wang
- "In-Memory Computing: Advances and Prospects" - IEEE Micro
- "Near-Data Processing: Insights from a Micro-46 Workshop" - IEEE Micro
