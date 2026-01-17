# Domain-Driven Design: ThermalBrain Domain Model

## Strategic Design

### Core Domain
**Neuromorphic Thermal Processing** - The unique value proposition of combining spiking neural networks with thermal self-regulation for embedded AI.

### Supporting Domains
- **Power Management** - DVFS, power gating, burst mode
- **Memory Management** - Sparse encoding, compression, arena allocation
- **Pattern Storage** - HNSW indexing, pattern matching

### Generic Domains
- **Configuration** - System parameters
- **Platform Abstraction** - Hardware interfaces

## Bounded Contexts

```
┌─────────────────────────────────────────────────────────────────┐
│                     ThermalBrain Context                        │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │   Neural    │  │   Thermal   │  │  Encoding   │             │
│  │   Context   │  │   Context   │  │   Context   │             │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘             │
│         │                │                │                     │
│         └────────────────┼────────────────┘                     │
│                          │                                      │
│                  ┌───────┴───────┐                              │
│                  │ Optimization  │                              │
│                  │   Context     │                              │
│                  └───────────────┘                              │
└─────────────────────────────────────────────────────────────────┘
```

## Aggregates

### 1. ThermalBrain (Root Aggregate)
The main entry point coordinating all subsystems.

```rust
ThermalBrain
├── ThermalGovernor      // Thermal management
├── FeatureExtractor     // Signal processing
├── SparseEncoder        // Sparse representation
├── SpikingMatcher       // Pattern matching
├── MiniHnsw             // Pattern index
└── SystemStatus         // Current state
```

### 2. Neural Aggregate
Manages neurons and spike processing.

```rust
NeuralAggregate
├── LIFNeuron[]          // Neuron bank
├── SpikeTrain[]         // Spike history
└── ConnectionMatrix     // Synaptic weights
```

### 3. Optimization Aggregate
Manages performance optimization strategies.

```rust
OptimizationAggregate
├── DvfsController       // Voltage/frequency
├── PowerGatingController // Power states
├── BurstController      // Burst mode
├── QuantizationEngine   // Precision control
└── EventDrivenProcessor // Async processing
```

## Entities

| Entity | Identity | Lifecycle |
|--------|----------|-----------|
| Pattern | pattern_id (u32) | Created on learn(), persists |
| Neuron | neuron_id (u16) | Created at init, long-lived |
| Connection | (source, target) | Dynamic, can be pruned |
| ThermalZone | zone enum | Stateless, derived from temp |

## Value Objects

| Value Object | Properties | Immutable |
|--------------|------------|-----------|
| PatternVector | [i8; 32] | Yes |
| SparseVector | indices, values | Yes |
| FeatureVector | [f32; 32] | Yes |
| SpikeTiming | neuron_id, time_us, phase | Yes |
| MatchResult | label, confidence, similarity | Yes |
| PerfLevel | freq_mult, voltage_scale, name | Yes |

## Domain Events

```rust
enum DomainEvent {
    // Thermal Events
    ZoneChanged { from: ThermalZone, to: ThermalZone, temp_c: f32 },
    EmergencyEntered { temp_c: f32 },
    EmergencyCleared { temp_c: f32 },

    // Neural Events
    SpikeGenerated { neuron_id: u16, time_us: u64 },
    PatternMatched { pattern_id: u32, confidence: f32 },
    PatternLearned { pattern_id: u32, label: String },

    // Power Events
    DvfsLevelChanged { from: usize, to: usize },
    BurstModeEntered,
    BurstModeExited,
    PowerStateChanged { bank_id: u8, state: PowerState },
}
```

## Repositories

### PatternRepository
```rust
trait PatternRepository {
    fn store(&mut self, pattern: PatternVector, label: &str) -> Result<u32>;
    fn find(&self, id: u32) -> Option<&PatternVector>;
    fn search(&mut self, query: &PatternVector, k: usize) -> Vec<(u32, f32)>;
    fn delete(&mut self, id: u32) -> Result<()>;
    fn count(&self) -> usize;
}
```

### NeuronRepository
```rust
trait NeuronRepository {
    fn get(&self, id: u16) -> Option<&LIFNeuron>;
    fn get_mut(&mut self, id: u16) -> Option<&mut LIFNeuron>;
    fn reset_all(&mut self);
    fn set_all_thresholds(&mut self, threshold: f32);
}
```

## Services

### InferenceService
Coordinates pattern matching workflow.

```rust
impl InferenceService {
    fn process(&mut self) -> Option<MatchResult> {
        // 1. Check thermal governor
        // 2. Extract features
        // 3. Encode to sparse
        // 4. Search patterns
        // 5. Fire matching neurons
        // 6. Return result
    }
}
```

### OptimizationService
Coordinates power/performance optimization.

```rust
impl OptimizationService {
    fn optimize(&mut self, load: f32, temp: f32) {
        // 1. Update DVFS based on load
        // 2. Update power gating
        // 3. Check burst conditions
        // 4. Adapt precision
    }
}
```

## Factories

### ThermalBrainFactory
```rust
impl ThermalBrainFactory {
    fn create_default() -> ThermalBrain;
    fn create_with_config(config: ThermalBrainConfig) -> ThermalBrain;
    fn create_low_power() -> ThermalBrain;
    fn create_high_performance() -> ThermalBrain;
}
```

## Anti-Corruption Layers

### PlatformAdapter
Translates between domain concepts and hardware-specific APIs.

```rust
trait PlatformAdapter {
    fn read_temperature(&self) -> f32;
    fn set_frequency(&self, multiplier: f32);
    fn set_voltage(&self, scale: f32);
    fn sleep_us(&self, microseconds: u64);
}
```

## Context Maps

```
┌──────────────┐         ┌──────────────┐
│   Neural     │◄───U────│   Thermal    │
│   Context    │         │   Context    │
└──────────────┘         └──────────────┘
       │                        │
       │    Shared Kernel       │
       └──────────┬─────────────┘
                  │
          ┌───────┴───────┐
          │ Optimization  │
          │   Context     │
          └───────────────┘
                  │
                  │ ACL
                  ▼
          ┌───────────────┐
          │   Platform    │
          │   Context     │
          └───────────────┘

Legend:
U = Upstream/Downstream
ACL = Anti-Corruption Layer
```

## Ubiquitous Language

| Term | Definition |
|------|------------|
| **Spike** | Discrete neural activation event |
| **Membrane Potential** | Accumulated charge in a neuron |
| **Threshold** | Activation level triggering a spike |
| **Refractory Period** | Cooldown after spiking |
| **Thermal Zone** | Temperature-based operating regime |
| **DVFS** | Dynamic Voltage and Frequency Scaling |
| **Power Gating** | Cutting power to idle circuits |
| **Burst Mode** | Temporary high-performance operation |
| **Pattern** | Learned feature representation |
| **Sparse Vector** | Mostly-zero representation |
