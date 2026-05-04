# ADR-016: Parking occupancy cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `parking-occupancy`

## Context

Real-time parking occupancy is high-value for retail (curb utilization),
office parks (capacity planning), and smart cities (curb pricing). Per-
spot vision sensors are expensive; per-zone CSI sensing is cheap and
accurate enough for occupied/free counts.

The seed's ESP32 ruview WiFi-CSI stream resolves "is there a vehicle
or person in this region of the antenna pattern" via subcarrier
amplitude shifts.

## Decision

`parking-occupancy` is a **ruview-required** cog. It runs at 0.2 Hz
(every 5 s — vehicles don't park faster than that) and tracks:

1. **N occupancy zones** — configurable, default 4 (typical small lot
   end-cap). Each zone is a subset of CSI subcarriers.
2. **Zone state** — occupied / free, hysteresis to avoid flicker.
3. **Counter** — total occupied, total free, churn-per-hour.

## CLI

```
cog-parking-occupancy [--once] [--interval 5] [--zones 4]
                      [--threshold 0.4]
```

## Output

```json
{
  "status": "monitoring",
  "occupied_count": 0,
  "total_zones": 4,
  "utilization_pct": 0.0,
  "churn_per_hour": 0.0,
  "zone_states": [false, false, false, false],
  "timestamp": 1730000000
}
```

## Consequences

### Positive
- Single seed covers ~6-12 spots.
- No per-spot hardware.
- Privacy: no images / video.

### Negative
- Cannot read license plates / detect specific vehicles.
- Coarse zone resolution.

## Alternatives considered
- **PIR per spot**. Rejected — many sensors, wiring nightmare.
- **Camera per zone**. Rejected — privacy, cost.

## RuView mode

**Required.** This cog refuses to run without ESP32 CSI input.

## Resource budget
- Binary: < 360 KB armhf.
- RAM: < 1.5 MB.
- CPU: < 1% at 0.2 Hz.

See ADR-001.
