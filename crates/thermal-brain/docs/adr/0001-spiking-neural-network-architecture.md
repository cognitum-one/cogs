# ADR-0001: Spiking Neural Network Architecture

## Status
Accepted

## Context
ThermalBrain requires an efficient neural processing architecture for embedded systems with severe memory and power constraints. Traditional artificial neural networks (ANNs) with continuous activations are computationally expensive and power-hungry.

## Decision
We adopt a **Spiking Neural Network (SNN)** architecture using Leaky Integrate-and-Fire (LIF) neurons.

### Key Design Choices

1. **LIF Neuron Model**
   - Membrane potential with exponential decay: `V(t+dt) = V(t) * exp(-dt/τ) + I(t)`
   - Discrete spike output when threshold reached
   - Refractory period prevents rapid re-firing

2. **Sparse Representation**
   - >90% sparsity target
   - Only non-zero values stored and processed
   - Sparse-dense multiplication for inference

3. **Thermal-Adaptive Thresholds**
   - Spike threshold varies with temperature zone
   - Cool: 0.30 (sensitive), Hot: 0.70 (selective), Emergency: 0.90 (minimal)

## Consequences

### Positive
- **Power Efficiency**: Spikes are binary events, reducing compute
- **Sparse Activity**: Most neurons inactive most of the time
- **Temporal Integration**: Natural handling of time-series data
- **Hardware Friendly**: Maps well to neuromorphic chips

### Negative
- **Training Complexity**: Gradient-based training is harder
- **Accuracy**: May have lower accuracy than ANNs for some tasks
- **Tooling**: Less mature tooling compared to ANN frameworks

## References
- Maass, W. (1997). Networks of spiking neurons: The third generation of neural network models
- SpiNNaker Project: https://apt.cs.manchester.ac.uk/projects/SpiNNaker/
