# ADR-0005: Mixed-Precision Quantization Strategy

## Status
Accepted

## Context
Full-precision (FP32) neural networks require 4 bytes per weight. For embedded systems with limited memory, this is often prohibitive. Quantization reduces precision to save memory and enable faster integer operations.

## Decision
Implement **Mixed-Precision Quantization** with INT4 weights and INT8 activations.

### Quantization Levels

| Type | Bits | Range | Memory Factor | Accuracy |
|------|------|-------|---------------|----------|
| INT4 | 4 | [-8, 7] | 0.125x | ~92% |
| INT8 | 8 | [-128, 127] | 0.25x | ~98% |
| INT16 | 16 | [-32768, 32767] | 0.5x | ~99.5% |
| FP32 | 32 | Full | 1.0x | 100% |

### Key Design Choices

1. **Calibration-Based Quantization**
   - Collect min/max from representative data
   - Compute optimal scale and zero-point
   - Symmetric quantization for simplicity

2. **Per-Layer Precision**
   - First/last layers: INT8 (higher sensitivity)
   - Middle layers: INT4 (lower sensitivity)
   - Adaptive adjustment based on error

3. **INT4 Packing**
   - Two 4-bit values per byte
   - 16 values in 8 bytes
   - Efficient SIMD unpacking

4. **Mixed-Precision Kernel**
   - INT4 weights × INT8 activations
   - INT16 accumulator to prevent overflow
   - Final scaling to output range

## Consequences

### Positive
- **50-87% Memory Reduction**: INT8/INT4 vs FP32
- **4x Compute Speed**: Integer ops faster than FP
- **SIMD Friendly**: Pack more values per register
- **Minimal Accuracy Loss**: <2% for INT8, <8% for INT4

### Negative
- **Calibration Required**: Need representative data
- **Quantization Error**: Some accuracy loss inevitable
- **Overflow Risk**: Careful accumulator sizing needed

## References
- Google: Quantization and Training of Neural Networks for Efficient Integer-Arithmetic-Only Inference
- NVIDIA: Integer Quantization for Deep Learning Inference
