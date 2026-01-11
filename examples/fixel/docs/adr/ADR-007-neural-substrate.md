# ADR-007: Neural Substrate - Native Support for Spiking and Continuous Neural Models

## Status

**Accepted**

## Date

2026-01-09

## Context

FIXEL's massively parallel architecture naturally maps to neural network computation. Two fundamental paradigms exist:

**Continuous (ANN) Models:**
- Standard deep learning (CNNs, transformers)
- Floating/fixed-point activations
- Synchronous layer-by-layer execution
- Well-established training and tooling

**Spiking (SNN) Models:**
- Neuromorphic computing paradigm
- Binary spike events over time
- Asynchronous, event-driven execution
- Lower energy for sparse activations

Market demands:
1. **Edge AI inference**: CNNs/transformers for vision, NLP (dominant today)
2. **Event cameras**: DVS sensors generate spike streams (growing market)
3. **Robotics**: Temporal processing, sensory-motor loops
4. **Scientific modeling**: Brain simulation, neural prosthetics

Competitive landscape:
- **GPUs**: Optimized for continuous models, poor spike efficiency
- **Intel Loihi**: Spike-native, limited continuous model support
- **IBM TrueNorth**: Spike-only, no standard DNN compatibility

FIXEL opportunity: Bridge both worlds in single architecture.

## Decision

We will implement **native support for both spiking and continuous neural models** within the cognitum architecture.

### Dual-Mode Architecture

```
COGNITUM NEURAL SUBSTRATE:

┌────────────────────────────────────────────────┐
│              MODE SELECTOR                      │
│        ┌─────────┴─────────┐                   │
│        ▼                   ▼                   │
│  ┌──────────┐       ┌──────────────┐          │
│  │CONTINUOUS│       │   SPIKING    │          │
│  │   MODE   │       │    MODE      │          │
│  └────┬─────┘       └──────┬───────┘          │
│       │                    │                   │
│  ┌────▼─────┐       ┌──────▼───────┐          │
│  │ MAC Unit │       │ LIF Neuron   │          │
│  │ 8x8→16   │       │ Integrate    │          │
│  └────┬─────┘       └──────┬───────┘          │
│       │                    │                   │
│  ┌────▼─────┐       ┌──────▼───────┐          │
│  │Activation│       │  Threshold   │          │
│  │ ReLU/etc │       │  + Spike     │          │
│  └────┬─────┘       └──────┬───────┘          │
│       │                    │                   │
│       ▼                    ▼                   │
│    8-bit              Spike Event              │
│   Output              (1-bit + timestamp)      │
└────────────────────────────────────────────────┘
```

### Continuous Mode Specification

```
ACTIVATION FUNCTIONS (LUT-based, 256 entries):
- ReLU:    max(0, x)           [1 cycle]
- Sigmoid: 1/(1+exp(-x))       [3 cycles, LUT]
- Tanh:    tanh(x)             [3 cycles, LUT]
- GELU:    x*Φ(x)              [5 cycles, approx]
- Swish:   x*sigmoid(x)        [4 cycles]

LAYER SUPPORT:
- Convolution: 1x1, 3x3, 5x5, 7x7 via neighbor access
- Pooling: Max, average (2x2, 4x4 via tile)
- BatchNorm: Folded into conv weights (inference)
- Attention: Via wave propagation + tile reduction
- Residual: Local addition with prior layer output

PRECISION:
- Weights: 8-bit signed
- Activations: 8-bit unsigned
- Accumulator: 16-bit signed
- Bias: 16-bit signed
```

### Spiking Mode Specification

```
NEURON MODEL: Leaky Integrate-and-Fire (LIF)

State per pixel:
- Membrane potential V: 16-bit fixed-point
- Refractory counter: 4-bit
- Threshold: 8-bit (programmable)
- Leak factor: 8-bit (programmable)

Update equation:
  V(t+1) = leak * V(t) + Σ(w_i * spike_i)
  if V > threshold:
      emit_spike()
      V = V_reset

SPIKE ENCODING:
- Format: 1-bit spike + 8-bit source ID + 16-bit timestamp
- Rate: Up to 1 spike per cycle per pixel
- Aggregate (8K): 3.32 trillion spikes/second max

SYNAPTIC MODELS:
- Instantaneous: Spike → immediate current injection
- Exponential: Spike → decaying current (3 cycles)
- Alpha: Spike → rise-fall profile (5 cycles)

PLASTICITY (optional):
- STDP: Spike-timing-dependent plasticity
- Window: ±20ms configurable
- Weight update: ±1 per correlated spike pair
```

### Hybrid Execution Mode

```python
@fixel_hybrid
def event_camera_to_classification(pixel: Pixel) -> None:
    """
    Hybrid pipeline:
    1. Receive spikes from DVS camera (spiking input)
    2. Convert to frame via temporal integration (S→C)
    3. Run CNN inference (continuous)
    4. Emit classification spikes (C→S)
    """
    # Spiking input stage
    pixel.set_mode(SPIKING)
    spike_count = 0
    for _ in range(integration_window):
        if pixel.receive_spike():
            spike_count += 1

    # Continuous processing stage
    pixel.set_mode(CONTINUOUS)
    frame_value = spike_count * scale_factor
    features = run_conv_layers(pixel, frame_value)
    class_score = run_fc_layer(pixel, features)

    # Spiking output stage
    pixel.set_mode(SPIKING)
    if class_score > threshold:
        pixel.emit_spike(class_id=class_score.argmax())
```

## Alternatives Considered

### Alternative 1: Pure ANN (Continuous Only)

**Pros:**
- Simpler hardware (no spike circuitry)
- Well-established tooling
- Direct PyTorch/TensorFlow model deployment
- Dominant market demand today

**Cons:**
- Cannot efficiently process event camera data
- Higher energy for sparse activations
- No path to neuromorphic computing
- Misses emerging brain-inspired computing market

**Rejected because:** FIXEL's unique architecture can efficiently support both paradigms. Limiting to ANN-only would forfeit significant competitive advantage in neuromorphic applications.

### Alternative 2: Pure SNN (Spiking Only)

**Pros:**
- Maximum energy efficiency for sparse data
- Natural fit for event-driven sensors
- True neuromorphic computing
- Lowest power for equivalent operations

**Cons:**
- Cannot run standard ML models
- Limited tooling (no PyTorch equivalent)
- Training is difficult (non-differentiable spikes)
- Small developer community
- Models must be converted or trained from scratch

**Rejected because:** The current AI ecosystem is built on continuous models. Pure SNN would exclude 99% of existing models and developers.

### Alternative 3: Separate Modes (No Hybrid)

**Pros:**
- Simpler per-pixel state machine
- No mode-switching overhead
- Easier verification

**Cons:**
- Cannot combine strengths in single pipeline
- Event cameras require full mode switch
- Redundant hardware for mode-specific operations
- Limited flexibility for mixed workloads

**Rejected because:** Real-world applications increasingly require mixed processing (event camera input, DNN processing, robotic control output). Separate modes would require inefficient data conversion at boundaries.

### Alternative 4: Emulated Spikes on Continuous Substrate

**Pros:**
- Single underlying computation model
- Simpler hardware
- Spikes as "rate-coded" continuous values

**Cons:**
- Loses energy advantage of true spikes
- High overhead for sparse spike patterns
- Cannot achieve true event-driven behavior
- Neuromorphic benchmarks would suffer

**Rejected because:** Emulation defeats the purpose of spike support. True spike hardware achieves 10-100x energy reduction for sparse patterns.

## Consequences

### Positive Consequences

1. **Market breadth**: FIXEL addresses both mainstream AI (continuous) and emerging neuromorphic (spiking) markets.

2. **Energy efficiency for sparse data**: Spike mode achieves >10x energy reduction for event cameras and sparse activations.

3. **Hybrid applications**: Event cameras can feed directly into CNN inference without external conversion.

4. **Future-proof**: Growing interest in brain-inspired computing positions FIXEL for long-term relevance.

5. **Scientific applications**: Brain simulation, neural prosthetics, and computational neuroscience become possible.

### Negative Consequences

1. **Hardware complexity**: Dual-mode requires additional transistors (~30K per pixel for spike circuitry).

2. **Verification burden**: Two execution modes multiply test cases and corner cases.

3. **Tooling investment**: Must develop spike-aware compilers, debuggers, and training frameworks.

4. **Mode-switching overhead**: Transitioning between modes requires state save/restore (100+ cycles).

5. **Developer learning curve**: Spike programming requires understanding of temporal dynamics.

### Performance Comparison

| Metric | Continuous Mode | Spiking Mode |
|--------|-----------------|--------------|
| Operations/cycle/pixel | 1 MAC | 1 spike + integrate |
| Energy per operation | 5 pJ | 0.5 pJ (sparse) |
| Latency (layer) | Deterministic | Data-dependent |
| Throughput (4K, 100MHz) | 830 TOps/s | 830T spikes/s |
| Best for | Dense matrices | Sparse events |

### Model Conversion Guidelines

**Continuous to Spiking:**
1. Replace ReLU with LIF threshold
2. Rate-code continuous values into spike trains
3. Increase temporal integration for accuracy
4. Typical accuracy loss: 1-5%

**Spiking to Continuous:**
1. Integrate spike counts over time window
2. Normalize by window length
3. No information loss with sufficient window

### Example: Complete SNN

```python
@fixel_spiking
def spiking_conv_layer(pixel: Pixel) -> None:
    """
    Spiking convolutional layer.
    Integrates spikes from neighbors, emits when threshold reached.
    """
    # Initialize membrane potential
    V = pixel.membrane_potential

    # Leak
    V = (V * pixel.leak_factor) >> 8

    # Integrate spikes from neighbors
    for neighbor in pixel.neighbors_3x3():
        if neighbor.has_spike():
            synapse_id = neighbor.source_id
            weight = pixel.sram[synapse_id]  # Synaptic weight
            V += weight

    # Fire if threshold exceeded
    if V > pixel.threshold:
        pixel.emit_spike()
        V = pixel.reset_potential
        pixel.set_refractory(pixel.refractory_period)

    # Update state
    pixel.membrane_potential = V
```

## Related Decisions

- ADR-001 (Cognitum Architecture): 16-bit accumulator supports membrane potential
- ADR-002 (Interconnect Topology): Mesh enables spike propagation
- ADR-006 (Programming Model): Dataflow variant maps to spike execution

## References

- "Deep Learning in Spiking Neural Networks" - Tavanaei et al., Neural Networks 2019
- "Intel Loihi Architecture" - Davies et al., IEEE Micro 2018
- "Neuromorphic Computing and Engineering" - Nature Reviews
- "SNN Training via Surrogate Gradients" - Neftci et al., IEEE Signal Processing 2019
