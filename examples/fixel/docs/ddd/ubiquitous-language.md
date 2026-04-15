# FIXEL Ubiquitous Language

## Glossary

This document defines the shared vocabulary used across all FIXEL documentation, code, and communication.

---

### A

**Accumulator**
: A 16-bit register in each Cognitum that accumulates partial products during MAC operations. Supports saturation arithmetic to prevent overflow.

**Activation Function**
: A non-linear transformation applied to Cognitum outputs. Supported functions: ReLU, Sigmoid, Tanh, Step. Implemented via 256-entry lookup tables.

---

### B

**Broadcast**
: The operation where a Tile Controller sends shared data (typically weights) to all Cognitums within its tile simultaneously. Takes 16 cycles.

---

### C

**Cognitum**
: The fundamental computational unit in FIXEL, embedded behind each pixel. Contains an 8-bit datapath, 16-bit accumulator, local SRAM, and spiking neuron logic. From Latin "cognitus" (known/understood).

**Convolution**
: A mathematical operation applying a kernel to neighboring pixels. FIXEL natively supports 3×3, 5×5, and NxN convolutions through mesh communication.

---

### D

**Datapath**
: The 8-bit arithmetic unit within each Cognitum. Performs integer operations: ADD, SUB, MUL, AND, OR, XOR, SHIFT.

**Density Tier**
: One of five hardware configurations defining the complexity/cost tradeoff:
- **NANO**: 64×64, minimal compute, IoT applications
- **MICRO**: 256×256, light inference, wearables
- **STANDARD**: 1920×1080, full neural inference, tablets
- **PRO**: 3840×2160, large models, professional displays
- **ULTRA**: 7680×4320, maximum capability, workstations

---

### F

**Fabric**
: The complete 2D grid of interconnected Cognitums forming the computational display. The primary aggregate root in the domain model.

**Fire (verb)**
: The action of a spiking neuron generating a spike event when membrane potential exceeds threshold.

**FIXEL**
: Portmanteau of "Finn" (creator) + "Pixel". The complete cognitive display system including hardware and software stack.

---

### I

**Inference**
: Running a trained neural network to produce predictions. FIXEL is optimized for inference, not training.

**Integrate (verb)**
: The process of accumulating input currents into a neuron's membrane potential.

**Interconnect**
: The communication links between Cognitums. Primary topology is 4-connected mesh (N, E, S, W).

---

### K

**Kernel**
: A small matrix of weights used in convolution operations. Common sizes: 3×3, 5×5, 7×7.

---

### L

**Latency**
: Time delay for an operation to complete. Measured in clock cycles.
- Neighbor access: 1 cycle
- Tile broadcast: 16 cycles
- DRAM access: 100+ cycles

**Leaky Integrate-and-Fire (LIF)**
: The spiking neuron model used in FIXEL. Membrane potential leaks over time and fires when threshold is reached.

---

### M

**MAC (Multiply-Accumulate)**
: The core operation of neural inference: acc += a × b. FIXEL performs 8×8→16-bit MAC in one cycle.

**Membrane Potential**
: An 8-bit value representing the electrical charge accumulated in a spiking neuron. Decays over time (leaky).

**Mesh Topology**
: The 2D grid interconnect where each Cognitum connects to its four orthogonal neighbors.

---

### P

**Pixel**
: A single display element that also contains a Cognitum. In FIXEL, pixels are both display and compute units.

**Power Budget**
: The maximum power consumption allowed per Cognitum, defined by density tier. Ranges from 0.01 µW (NANO) to 1.2 µW (ULTRA).

**Propagate**
: The spreading of spike events from firing neurons to connected neighbors, weighted by synaptic connections.

---

### R

**Reduction**
: Combining multiple values into one using an associative operation (sum, max, min, mean). Performed hierarchically through tiles.

**Refractory Period**
: The time (in cycles) after firing during which a neuron cannot fire again. Prevents runaway excitation.

---

### S

**Saturation Arithmetic**
: Arithmetic that clamps results to valid range instead of overflowing. INT8: [-128, 127], INT16: [-32768, 32767].

**Spike**
: A discrete event representing neural activation. Contains source position, timestamp, and amplitude.

**SRAM**
: Static Random-Access Memory embedded in each Cognitum. Size varies by tier: 16B (NANO) to 1KB (ULTRA).

---

### T

**Threshold**
: The membrane potential level that triggers a spike. Configurable per Cognitum, typically 128-200.

**Tile**
: A 16×16 group of Cognitums sharing memory resources and controlled by a single tile controller.

**Tile Controller**
: Hardware managing shared memory, broadcasts, and reductions within a tile.

---

### W

**Weight**
: A numerical parameter in neural networks defining connection strength. Stored in tile shared memory for efficient reuse.

**Weight Sharing**
: The CNN technique of using the same weights across spatial locations. Enabled by tile broadcast.

---

### X

**XY Routing**
: The deadlock-free routing algorithm used in the mesh. Routes X direction first, then Y direction.
