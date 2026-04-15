# ADR-005: Power Management - Per-Tile Clock Gating with Adaptive Voltage

## Status

**Accepted**

## Date

2026-01-09

## Context

FIXEL must operate across extreme power envelopes:

| Scenario | Total Budget | Per-Pixel Budget | Use Case |
|----------|--------------|------------------|----------|
| Battery (tablet) | 5W | 0.6 uW | Portable 4K |
| Wall (monitor) | 50W | 1.5 uW | Desktop 8K |
| Workstation | 200W | 6 uW | High-performance 8K |
| Datacenter | 500W | 15 uW | Maximum throughput |

Power consumption components per pixel:
- **Dynamic**: Switching activity (CV^2f)
- **Leakage**: Static current through transistors
- **Interconnect**: Driving mesh links
- **Display driver**: LED/OLED output stage

At 2nm, leakage is significant (~30-40% of active power) due to:
- Reduced threshold voltage
- Increased subthreshold leakage
- Gate leakage through thin oxides

Key requirements:
1. **Wide dynamic range**: 10-100x power scaling based on workload
2. **Fast response**: Adapt within microseconds for bursty workloads
3. **Thermal uniformity**: Prevent hotspots during localized activity
4. **Graceful degradation**: Reduce capability rather than fail
5. **Display continuity**: Power management must not corrupt display output

## Decision

We will implement **per-tile clock gating with adaptive voltage scaling (AVS)**.

### Architecture

```
POWER DOMAINS:

Level 0: Pixel (finest grain)
- Power gating not practical (too much overhead)
- Clock gating via local clock enable
- Sleep mode: Hold state, disable clock

Level 1: Micro-Tile (4x4 = 16 pixels)
- Clock gating with single enable signal
- Shared clock tree
- Wake latency: 2 cycles

Level 2: Tile (16x16 = 256 pixels)
- Clock gating + power gating option
- Voltage domain boundary
- Retention SRAM for sleep state
- Wake latency: 100 cycles

Level 3: Super-Tile (64x64 = 4096 pixels)
- Voltage/frequency island
- Independent AVS controller
- Deep sleep with full power down
- Wake latency: 10,000 cycles
```

### Voltage Scaling Modes

```
MODE 1: MINIMUM (0.4V)
- Frequency: 10-50 MHz
- Power: 0.3 uW/pixel
- Performance: 10 MOps/s/pixel
- Use: Static display, idle

MODE 2: EFFICIENT (0.5V)
- Frequency: 100 MHz
- Power: 0.6 uW/pixel
- Performance: 100 MOps/s/pixel
- Use: Video playback, light inference

MODE 3: BALANCED (0.6V)
- Frequency: 200 MHz
- Power: 1.5 uW/pixel
- Performance: 200 MOps/s/pixel
- Use: Real-time inference

MODE 4: PERFORMANCE (0.7V)
- Frequency: 500 MHz
- Power: 4.0 uW/pixel
- Performance: 500 MOps/s/pixel
- Use: Neural video generation

MODE 5: MAXIMUM (0.8V)
- Frequency: 1 GHz
- Power: 10 uW/pixel
- Performance: 1 GOps/s/pixel
- Use: Burst compute, thermal limited
```

### Control Hierarchy

```
Display Controller
    ├── Global power budget
    ├── Thermal limits
    └── Workload hints
         │
         ▼
Super-Tile AVS Controller (1 per 4096 pixels)
    ├── Local temperature sensor
    ├── Voltage regulator control
    └── Performance demand aggregation
         │
         ▼
Tile Clock Controller (1 per 256 pixels)
    ├── Clock enable/disable
    ├── Activity monitoring
    └── Sleep state management
         │
         ▼
Pixel Clock Gate (per pixel)
    └── Local clock enable
```

### Adaptive Voltage Algorithm

```python
def adaptive_voltage_control(super_tile):
    # Sample every 1ms
    temperature = super_tile.read_temp_sensor()
    activity = super_tile.get_activity_level()  # 0.0 to 1.0
    demand = super_tile.get_pending_operations()

    # Thermal throttling
    if temperature > TEMP_CRITICAL:  # 100C
        super_tile.set_voltage(V_MINIMUM)
        return
    elif temperature > TEMP_HIGH:     # 85C
        max_voltage = V_BALANCED
    else:
        max_voltage = V_MAXIMUM

    # Demand-based scaling
    if demand > DEMAND_HIGH:
        target = min(V_PERFORMANCE, max_voltage)
    elif demand > DEMAND_MEDIUM:
        target = min(V_BALANCED, max_voltage)
    elif demand > DEMAND_LOW:
        target = min(V_EFFICIENT, max_voltage)
    else:
        target = V_MINIMUM

    # Ramp voltage (avoid instant transitions)
    super_tile.ramp_voltage(target, rate=10mV_per_us)
```

## Alternatives Considered

### Alternative 1: Global Power States Only

**Pros:**
- Simple implementation (single voltage domain)
- Uniform behavior across display
- Straightforward programming model

**Cons:**
- Cannot optimize for localized activity
- Entire display at performance or efficient, not mixed
- Thermal hotspots from active regions
- Poor efficiency for partial-display workloads (PiP, focus areas)

**Rejected because:** Real workloads exhibit spatial locality (user looking at one region, active window, etc.). Global-only power management wastes energy on inactive regions.

### Alternative 2: Per-Pixel Power Gating

**Pros:**
- Maximum granularity
- Optimal power for any activity pattern
- No wasted leakage

**Cons:**
- Power switch per pixel consumes 5-10% area
- Control signal routing to 33.2M pixels
- Wake latency per pixel (1000+ cycles)
- Retention registers per pixel add overhead

**Rejected because:** The overhead of per-pixel power switches exceeds the energy savings for typical workloads. Tile-level granularity captures 90%+ of the benefit at 1% of the cost.

### Alternative 3: Fixed Voltage with Clock Gating Only

**Pros:**
- No voltage regulator complexity
- Fast clock enable/disable
- Predictable timing

**Cons:**
- Voltage dominates power (P ~ V^2)
- Limited dynamic range (3-5x vs 10-100x)
- Leakage unchanged when clocks off
- Poor battery life

**Rejected because:** Clock gating alone cannot achieve the power reduction needed for battery-powered operation. Voltage scaling provides quadratic power reduction.

### Alternative 4: Per-Pixel Voltage Scaling

**Pros:**
- Maximum optimization granularity
- Could match activity exactly
- Theoretical minimum energy

**Cons:**
- 33.2M voltage regulators required
- Impossible with current technology
- Massive area overhead
- Extreme control complexity

**Rejected because:** Physically impossible to implement voltage regulators at pixel scale. Super-tile granularity (4096 pixels) is the practical minimum.

## Consequences

### Positive Consequences

1. **Wide power envelope**: 10-100x dynamic range enables battery and datacenter operation from same silicon.

2. **Thermal management**: Per-tile temperature sensing and throttling prevents damage and ensures reliability.

3. **Efficient partial use**: Picture-in-picture, focused attention, and partial-screen updates only power active regions.

4. **Fast response**: Clock gating at tile level enables microsecond-scale power adaptation for bursty workloads.

5. **Graceful degradation**: Under thermal stress, system reduces performance rather than failing.

### Negative Consequences

1. **Voltage regulator area**: Super-tile voltage regulators consume silicon that could be used for compute or memory.

2. **Transition latency**: Voltage changes require 10-100us to stabilize; cannot instantly boost for single operations.

3. **Complexity**: Multi-level power hierarchy adds verification burden and potential failure modes.

4. **Non-uniform performance**: Different tiles may operate at different voltages, complicating performance guarantees.

### Power Breakdown by Component

| Component | Active (%) | Clock Gated (%) | Power Gated (%) |
|-----------|------------|-----------------|-----------------|
| Compute | 35% | 0% | 0% |
| SRAM | 25% | 10% | 0% |
| Interconnect | 15% | 5% | 0% |
| Clock tree | 10% | 0% | 0% |
| Leakage | 15% | 15% | 0% |
| Display driver | - | - | - |

### Transition Times

| Transition | Latency | Energy Overhead |
|------------|---------|-----------------|
| Clock gate (tile) | 2 cycles | 0.1 pJ/pixel |
| Clock ungate (tile) | 2 cycles | 0.1 pJ/pixel |
| Voltage up (1 step) | 10 us | 10 pJ/pixel |
| Voltage down (1 step) | 10 us | 5 pJ/pixel |
| Power gate (tile) | 100 cycles | 50 pJ/pixel |
| Power ungate (tile) | 1000 cycles | 100 pJ/pixel |

## Related Decisions

- ADR-001 (Cognitum Architecture): Operating voltage range affects transistor sizing
- ADR-003 (Memory Hierarchy): SRAM retention mode for low-power sleep
- ADR-004 (Density Tiers): Power budget defines tier capabilities

## References

- "Sub-threshold Design for Ultra Low-Power Systems" - Wang, Chandrakasan, Kosonocky
- "Adaptive Body Biasing for Reducing Impacts of Die-to-Die and Within-Die Parameter Variations" - Tschanz et al.
- "Power Management Techniques for Datacenters" - Google/Intel research
