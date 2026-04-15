# ADR-0003: Dynamic Voltage and Frequency Scaling (DVFS)

## Status
Accepted

## Context
Power consumption in digital circuits follows: `P ∝ V² × F`

By dynamically adjusting voltage and frequency, we can achieve significant power savings while maintaining performance when needed.

## Decision
Implement **SpiNNaker-2 style DVFS** with 8 performance levels and per-core granularity.

### Performance Levels

| Level | Name | Freq Mult | Voltage | Power Factor |
|-------|------|-----------|---------|--------------|
| 0 | ultra_low | 0.125x | 0.60V | 0.045 |
| 1 | very_low | 0.25x | 0.70V | 0.123 |
| 2 | low | 0.50x | 0.80V | 0.320 |
| 3 | medium_low | 0.75x | 0.90V | 0.608 |
| 4 | nominal | 1.00x | 1.00V | 1.000 |
| 5 | medium_high | 1.25x | 1.05V | 1.378 |
| 6 | high | 1.50x | 1.10V | 1.815 |
| 7 | turbo | 2.00x | 1.20V | 2.880 |

### Key Design Choices

1. **Ramping**
   - Up: 4 levels/second (fast response)
   - Down: 2 levels/second (prevents oscillation)

2. **Load-Based Scaling**
   - Upscale threshold: 75% load
   - Downscale threshold: 25% load

3. **Thermal Override**
   - Above throttle temperature → force to minimum
   - 5°C hysteresis for exit

4. **Burst Mode Integration**
   - Time-limited maximum performance
   - Automatic cooldown after burst

## Consequences

### Positive
- **75% Power Reduction**: At lower performance levels
- **<100ns Transitions**: Fast response to workload changes
- **Fine-Grained Control**: 8 levels for precise tuning
- **Thermal Integration**: Automatic throttling

### Negative
- **Complexity**: More states to manage
- **Voltage Regulator**: Requires capable power supply
- **Timing Sensitivity**: Some operations may be timing-critical

## References
- IEEE: Dynamic Power Management for Neuromorphic Many-Core Systems
- SpiNNaker-2 Power Management Architecture
