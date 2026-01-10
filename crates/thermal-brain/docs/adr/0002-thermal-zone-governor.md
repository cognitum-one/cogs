# ADR-0002: Thermal Zone Governor

## Status
Accepted

## Context
Embedded systems operating continuously can overheat, causing:
- Hardware damage
- Thermal throttling by the OS/firmware
- Reduced lifespan
- Unreliable behavior

We need a self-regulating thermal management system that adapts processing to temperature.

## Decision
Implement a **5-Zone Thermal Governor** with hysteresis-based state machine.

### Thermal Zones

| Zone | Temperature | Behavior |
|------|-------------|----------|
| Cool | < 40°C | Full performance |
| Warm | 40-50°C | Reduced activity |
| Hot | 50-60°C | Aggressive throttling |
| Critical | 60-70°C | Minimal processing |
| Emergency | > 70°C | Processing halted |

### Key Design Choices

1. **Hysteresis** (2°C default)
   - Prevents oscillation at zone boundaries
   - Must drop 2°C below threshold to return to lower zone

2. **Exponential Moving Average (EMA)**
   - Smooths temperature readings
   - Alpha = 0.1 for stability, 1.0 for instant response

3. **Per-Zone Parameters**
   - Spike threshold
   - Refractory period
   - Sleep duration
   - CPU scaling factor

## Consequences

### Positive
- **Self-Protection**: Prevents thermal damage
- **Graceful Degradation**: Performance scales with thermal budget
- **Predictable**: Clear zone boundaries and behaviors
- **Energy Efficient**: Reduces power when hot

### Negative
- **Reduced Throughput**: Processing limited when hot
- **Latency Variation**: Response time depends on zone
- **Configuration**: Zone thresholds may need per-device tuning

## References
- Linux Thermal Framework: https://www.kernel.org/doc/Documentation/thermal/
- SpiNNaker-2 Thermal Management
