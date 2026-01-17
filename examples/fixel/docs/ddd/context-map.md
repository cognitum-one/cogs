# FIXEL Context Map

## Bounded Contexts Overview

The FIXEL system is divided into four bounded contexts, each with its own domain model and responsibility.

```
┌─────────────────────────────────────────────────────────────────────┐
│                        FIXEL System                                  │
│                                                                      │
│  ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐ │
│  │                 │    │                 │    │                 │ │
│  │  HOST INTERFACE │    │    COMPUTE      │    │    DISPLAY      │ │
│  │    CONTEXT      │◄──►│    CONTEXT      │───►│    CONTEXT      │ │
│  │                 │    │                 │    │                 │ │
│  │  - Weight Load  │    │  - Cognitum     │    │  - Pixel Output │ │
│  │  - Image Input  │    │  - Fabric       │    │  - Refresh      │ │
│  │  - Results      │    │  - Tile         │    │  - Timing       │ │
│  │  - Control      │    │  - Spike        │    │  - Color Space  │ │
│  │                 │    │                 │    │                 │ │
│  └────────┬────────┘    └────────┬────────┘    └─────────────────┘ │
│           │                      │                                  │
│           │                      │                                  │
│           │    ┌─────────────────┴─────────────────┐               │
│           │    │                                   │               │
│           └───►│      POWER MANAGEMENT             │               │
│                │         CONTEXT                   │               │
│                │                                   │               │
│                │  - Thermal Monitoring             │               │
│                │  - Clock Gating                   │               │
│                │  - Tier Enforcement               │               │
│                │  - Energy Budgets                 │               │
│                │                                   │               │
│                └───────────────────────────────────┘               │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Context Descriptions

### 1. Compute Context (Core Domain)

**Responsibility:** Neural computation and spike propagation

**Key Entities:**
- Cognitum - Per-pixel compute unit
- Fabric - 2D grid aggregate
- Tile - 16×16 resource sharing group
- Spike - Neural activation event

**Language:**
- Uses terms: integrate, fire, membrane, threshold, propagate
- Operates in cycles and timesteps
- Values are quantized INT8/INT16

### 2. Host Interface Context

**Responsibility:** External communication and control

**Key Entities:**
- WeightLoader - DMA weight transfer
- ImageStream - Input data pipeline
- ResultCollector - Output aggregation
- CommandProcessor - Host commands

### 3. Display Context

**Responsibility:** Visual output generation

**Key Entities:**
- PixelDriver - Signal generation
- RefreshController - Timing management
- ColorSpace - RGB/YUV conversion
- Backlight - Illumination control

### 4. Power Management Context

**Responsibility:** Energy efficiency and thermal safety

**Key Entities:**
- PowerMonitor - Per-tile power tracking
- ThermalSensor - Temperature monitoring
- ClockGate - Dynamic frequency control
- TierEnforcer - Budget compliance

## Context Relationships

### 1. Compute ↔ Host Interface: Customer-Supplier

```
┌──────────────────┐         ┌──────────────────┐
│  Host Interface  │         │     Compute      │
│    (Supplier)    │────────►│   (Customer)     │
│                  │         │                  │
│  Provides:       │         │  Consumes:       │
│  - Weights       │         │  - Weights       │
│  - Input images  │         │  - Input data    │
│  - Commands      │         │  - Commands      │
│                  │         │                  │
│  Receives:       │◄────────│  Produces:       │
│  - Results       │         │  - Classifications│
│  - Status        │         │  - Metrics       │
└──────────────────┘         └──────────────────┘

Anti-Corruption Layer: WeightTranslator
- Converts host float32 weights to INT8
- Handles layout transformations
- Validates weight ranges
```

### 2. Compute → Display: Conformist

Display context conforms to Compute's output format. No translation layer needed.

### 3. Power Management ↔ All: Shared Kernel

```
┌────────────────────────────────────────────────────┐
│              Shared Kernel: PowerBudget            │
│                                                    │
│  Shared Types:                                     │
│  - PowerBudget { perPixel: µW, total: mW }        │
│  - ThermalLimit { warning: °C, critical: °C }     │
│  - EnergyReport { consumed: mJ, efficiency: % }   │
└────────────────────────────────────────────────────┘
```

## Strategic Context Map

```
                    ┌─────────────────────────┐
                    │     External World      │
                    │   (Applications/Users)  │
                    └───────────┬─────────────┘
                                │
                    ┌───────────▼─────────────┐
                    │    HOST INTERFACE       │
                    │      [Gateway]          │
                    │                         │
                    │  ACL: WeightTranslator  │
                    │  ACL: CommandParser     │
                    └───────────┬─────────────┘
                                │
              ┌─────────────────┴─────────────────┐
              │                                   │
    ┌─────────▼─────────┐           ┌─────────────▼─────────┐
    │     COMPUTE       │           │   POWER MANAGEMENT    │
    │   [Core Domain]   │◄─────────►│   [Supporting]        │
    │                   │  Shared   │                       │
    │  Fabric           │  Kernel   │  PowerMonitor         │
    │  Cognitum         │           │  ThermalSensor        │
    │  Tile             │           │  ClockGate            │
    └─────────┬─────────┘           └───────────────────────┘
              │
              │ Conformist
              │
    ┌─────────▼─────────┐
    │     DISPLAY       │
    │   [Supporting]    │
    │                   │
    │  PixelDriver      │
    │  RefreshController│
    └───────────────────┘
```

## Team Topologies

| Context | Team | Communication |
|---------|------|---------------|
| Compute | Core Architecture | Owns domain model, API contracts |
| Host Interface | System Integration | Implements ACL, manages protocols |
| Display | Display Engineering | Conforms to Compute output |
| Power Management | Power Engineering | Maintains shared kernel |
