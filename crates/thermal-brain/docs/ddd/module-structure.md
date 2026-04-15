# Domain-Driven Design: Module Structure

## Package Organization

```
thermal-brain/
├── src/
│   ├── lib.rs              # Public API (Application Service)
│   │
│   ├── config/             # Configuration (Value Objects)
│   │   └── mod.rs
│   │
│   ├── types/              # Domain Types (Entities, Value Objects)
│   │   └── mod.rs
│   │
│   ├── error/              # Domain Errors
│   │   └── mod.rs
│   │
│   ├── encoding/           # Encoding Context
│   │   ├── mod.rs
│   │   ├── feature.rs      # Feature Extraction Service
│   │   └── sparse.rs       # Sparse Encoding Service
│   │
│   ├── neural/             # Neural Context
│   │   ├── mod.rs
│   │   ├── lif.rs          # LIF Neuron Entity
│   │   ├── matcher.rs      # Spiking Matcher Service
│   │   └── hnsw.rs         # HNSW Repository
│   │
│   ├── governor/           # Thermal Context
│   │   ├── mod.rs
│   │   └── thermal.rs      # Thermal Governor Entity
│   │
│   ├── optimization/       # Optimization Context
│   │   ├── mod.rs
│   │   ├── dvfs.rs         # DVFS Service
│   │   ├── power_gating.rs # Power Gating Service
│   │   ├── burst_mode.rs   # Burst Mode Service
│   │   ├── quantization.rs # Quantization Service
│   │   ├── simd_ops.rs     # SIMD Operations (Infrastructure)
│   │   ├── adaptive_precision.rs
│   │   ├── network_pruning.rs
│   │   ├── spike_compression.rs
│   │   ├── delta_encoding.rs
│   │   ├── memory_arena.rs
│   │   ├── event_driven.rs
│   │   ├── temporal_coding.rs
│   │   ├── predictive_thermal.rs
│   │   └── meta_plasticity.rs
│   │
│   ├── platform/           # Platform Context (ACL)
│   │   ├── mod.rs
│   │   ├── esp32.rs
│   │   ├── cognitum.rs
│   │   └── wasm.rs
│   │
│   └── wasm/               # WASM Bindings (Interface)
│       └── mod.rs
│
├── docs/
│   ├── adr/                # Architecture Decision Records
│   └── ddd/                # Domain-Driven Design Docs
│
└── tests/                  # Integration Tests
```

## Layer Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Interface Layer                          │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │  lib.rs     │  │  wasm/      │  │  Platform   │         │
│  │  Public API │  │  Bindings   │  │  Adapters   │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
├─────────────────────────────────────────────────────────────┤
│                   Application Layer                         │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  ThermalBrain (Facade/Application Service)          │   │
│  │  - Coordinates all bounded contexts                 │   │
│  │  - Manages lifecycle and state                      │   │
│  └─────────────────────────────────────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                     Domain Layer                            │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌────────────┐    │
│  │ encoding │ │  neural  │ │ governor │ │optimization│    │
│  │ context  │ │ context  │ │ context  │ │  context   │    │
│  └──────────┘ └──────────┘ └──────────┘ └────────────┘    │
├─────────────────────────────────────────────────────────────┤
│                  Infrastructure Layer                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐         │
│  │  platform/  │  │  config/    │  │  types/     │         │
│  │  Hardware   │  │  Settings   │  │  Shared     │         │
│  └─────────────┘  └─────────────┘  └─────────────┘         │
└─────────────────────────────────────────────────────────────┘
```

## Dependency Rules

1. **Interface → Application**: UI/API calls application services
2. **Application → Domain**: Services use domain entities
3. **Domain → Domain**: Contexts can reference shared types
4. **Domain → Infrastructure**: Via dependency inversion (traits)
5. **Infrastructure → Nothing**: Lowest level, no dependencies up

## Module Responsibilities

### `lib.rs` (Application Service)
- Public API entry point
- Coordinates bounded contexts
- Manages transaction boundaries
- Enforces business rules

### `encoding/` (Encoding Context)
- Feature extraction from raw signals
- Sparse representation encoding
- Ring buffer management
- Statistical computation

### `neural/` (Neural Context)
- LIF neuron simulation
- Spike generation and propagation
- Pattern matching
- HNSW indexing

### `governor/` (Thermal Context)
- Temperature monitoring
- Zone state machine
- Thermal event generation
- Throttling decisions

### `optimization/` (Optimization Context)
- DVFS control
- Power gating
- Burst mode management
- Quantization
- Event-driven processing
- Compression

### `platform/` (Infrastructure)
- Hardware abstraction
- Platform-specific implementations
- Timer and sleep functions
- I/O operations

## Cross-Cutting Concerns

### Error Handling
- `ThermalBrainError` enum in `error/`
- Result types for fallible operations
- No panics in production code

### Configuration
- `ThermalBrainConfig` in `config/`
- Per-context configuration structs
- Builder pattern for construction

### Logging/Tracing
- Domain events for state changes
- Metrics collection for monitoring
- Compile-time feature flags

## Testing Strategy

### Unit Tests
- Per-module `#[cfg(test)]` sections
- Test each bounded context in isolation
- Mock dependencies via traits

### Integration Tests
- `tests/` directory
- Test context interactions
- Full workflow scenarios

### Property Tests
- Invariant verification
- Edge case discovery
- Fuzzing for robustness
