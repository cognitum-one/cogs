# ADR-001: Cognitum Core Architecture - 8-bit Datapath with 16-bit Accumulator

## Status

**Accepted**

## Date

2026-01-09

## Context

The FIXEL architecture places a "cognitum" chip behind every pixel, creating a massively parallel cognitive fabric with up to 33.2 million compute nodes (8K resolution). Each cognitum operates within an extremely constrained silicon budget of approximately 0.01 mm^2 at the 2nm process node, yielding roughly 2.3 million transistors per pixel.

The primary workloads for FIXEL are:
- Neural network inference (convolutions, activations, pooling)
- Real-time image processing (edge detection, filtering, enhancement)
- Spiking neural networks (neuromorphic computing)
- Cellular automata and simulation workloads

Key constraints driving this decision:
1. **Power budget**: 0.6-6 microW per pixel depending on use case (battery to workstation)
2. **Transistor budget**: ~2.3M transistors total, with SRAM consuming ~65%
3. **Latency requirements**: Sub-millisecond inference for real-time vision
4. **Manufacturing yield**: Simpler designs have higher yield at scale

The precision requirements for target workloads vary:
- Neural networks: INT8 is widely adopted and sufficient for inference
- Image processing: 8-bit color depth is standard
- Scientific computing: Often requires higher precision (FP16 or FP32)

## Decision

We will implement an **8-bit datapath with a 16-bit accumulator** for the cognitum compute core.

### Specification

```
Compute Unit Design:
- ALU: 8-bit operations (ADD, SUB, MUL, AND, OR, XOR, shifts)
- MAC: 8-bit x 8-bit -> 16-bit accumulate
- Accumulator: 16-bit for intermediate results
- Registers: 64 bytes (16 x 32-bit or 32 x 16-bit addressable)
- Activation functions: 8-bit LUT-based (ReLU, sigmoid, tanh)
```

### Transistor Allocation

| Component | Transistors | Percentage |
|-----------|-------------|------------|
| 8-bit MAC Unit | 50,000 | 2.2% |
| 8-bit ALU | 20,000 | 0.9% |
| 16-bit Accumulator | 10,000 | 0.4% |
| Control Logic | 30,000 | 1.3% |
| Total Compute | ~110,000 | 4.8% |

## Alternatives Considered

### Alternative 1: 4-bit Datapath

**Pros:**
- Lower power consumption (~50% reduction)
- Smaller silicon area, more room for SRAM
- Sufficient for some neural network models (INT4 quantization)

**Cons:**
- Inadequate for standard image processing (8-bit color)
- Limited neural network accuracy; requires specialized quantization
- Two cycles needed for standard 8-bit operations
- Poor match to display color depth (8 bits per channel)

**Rejected because:** The natural alignment with 8-bit pixel values and standard neural network quantization makes 4-bit impractical for general-purpose use.

### Alternative 2: 16-bit Datapath

**Pros:**
- Higher precision for scientific computing
- Eliminates need for separate accumulator width
- Better dynamic range for complex operations

**Cons:**
- Doubles ALU transistor count (~40,000 transistors)
- Increases power consumption by 60-80%
- Unnecessary precision for target workloads
- Reduces available transistors for SRAM

**Rejected because:** The additional precision provides minimal benefit for neural network and image processing workloads while significantly increasing power and area.

### Alternative 3: Mixed Precision (4/8/16 configurable)

**Pros:**
- Maximum flexibility for different workloads
- Can optimize precision per layer/operation
- Future-proof for emerging techniques

**Cons:**
- Significant control logic overhead (additional 30-50K transistors)
- Complex instruction encoding
- Higher verification complexity
- Power overhead for mode switching

**Rejected because:** The complexity overhead outweighs the benefits given the constrained transistor budget. The 8-bit with 16-bit accumulator provides sufficient flexibility.

## Consequences

### Positive Consequences

1. **Optimal for neural networks**: INT8 is the de facto standard for edge inference, with proven accuracy across major model architectures (ResNet, MobileNet, EfficientNet).

2. **Natural alignment with display**: 8-bit datapath directly matches 8-bit color channels, enabling zero-overhead pixel value manipulation.

3. **Power efficiency**: 8-bit operations at near-threshold voltage achieve ~5 pJ per MAC, matching neuromorphic efficiency.

4. **Sufficient accumulator range**: 16-bit accumulator supports dot products of up to 256 8-bit multiplications without overflow, adequate for 3x3 and 5x5 convolution kernels.

5. **Silicon simplicity**: Straightforward design reduces verification effort and improves manufacturing yield.

### Negative Consequences

1. **Limited scientific computing**: FP32 simulations (fluid dynamics, molecular modeling) cannot run natively. Must use fixed-point approximations or external compute.

2. **Precision ceiling**: Some neural network models requiring FP16 accumulation (attention mechanisms, BatchNorm) may lose accuracy.

3. **No native floating-point**: Applications requiring true floating-point must emulate, incurring significant cycle overhead.

4. **Quantization requirement**: Models must be quantized to INT8 before deployment, adding toolchain complexity.

### Mitigation Strategies

1. **External offload**: Display controller can handle high-precision operations for hybrid workloads.

2. **Tile-level aggregation**: Super-tiles can implement higher-precision operations through cooperative computation.

3. **LUT-based functions**: Complex functions (division, square root, transcendentals) implemented as lookup tables where precision permits.

## Related Decisions

- ADR-003 (Memory Hierarchy): SRAM sizing influenced by compute width
- ADR-006 (Programming Model): ISA design based on 8-bit datapath
- ADR-007 (Neural Substrate): Spike encoding uses 8-bit weights

## References

- "Quantization and Training of Neural Networks for Efficient Integer-Arithmetic-Only Inference" - Jacob et al., CVPR 2018
- "FINN: A Framework for Fast, Scalable Binarized Neural Network Inference" - Umuroglu et al.
- FIXEL Technical Specification v1.0
