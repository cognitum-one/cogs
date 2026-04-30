# ADR-013: Frost warning cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `frost-warning`

## Context

Frost is a leading cause of agricultural crop loss for stone fruits,
citrus, vegetables, and ornamentals. Forecasting frost the night
before requires more than current temperature — dewpoint depression
and wind speed matter at least as much.

A seed mounted in a field/orchard with a temperature/humidity sensor
ESP32 companion can predict frost 6–12 hours ahead by tracking:

1. **Temperature trend** — slope over last 4 h.
2. **Dewpoint depression** — (temp - dewpoint), proxies water vapor.
3. **Clear-sky proxy** — sustained low CSI variance (no air
   movement / convection).

When the forecast model says "below freezing within 6 h with
high confidence", `frost-warning` fires.

## Decision

`frost-warning` runs at 0.1 Hz (every 10 s) and uses a simple linear
extrapolation:

1. Sliding window of last 4 hours of temperature samples (proxied via
   ESP32 feature channel 0 if labeled `temp_c`, else proxy from
   variance baseline).
2. Compute slope; project 6 h ahead.
3. If projection < `frost_threshold_c` AND dewpoint depression
   < `dewpoint_min_depression`, fire `FROST_LIKELY`.
4. If current temp < `frost_threshold_c`, fire `FROST_CONFIRMED`.

## CLI

```
cog-frost-warning [--once] [--interval 10] [--frost-threshold 2.0]
                  [--projection-hours 6] [--dewpoint-min-depression 3.0]
```

## Output

```json
{
  "status": "warm|cooling|FROST_LIKELY|FROST_CONFIRMED",
  "frost_likely": false,
  "frost_confirmed": false,
  "current_temp_c": 0.0,
  "projected_temp_c_at_h": 0.0,
  "trend_c_per_h": 0.0,
  "dewpoint_depression_c": 0.0,
  "timestamp": 1730000000
}
```

## Consequences

### Positive
- Lead time for orchard heaters / sprinkler irrigation.
- 4 h sliding window is robust to single-sample noise.

### Negative
- Doesn't predict radiative frost without humidity sensor.
- Linear projection misses wind-driven warm-air-advection events.

## Alternatives considered
- **NWS API integration**. Out of scope — needs WiFi, credentialed
  weather API; cog is offline-first.
- **Surface temperature radiometer**. Out of scope — separate hardware.

## Resource budget
- Binary: < 320 KB armhf.
- RAM: < 1 MB (4 h sliding window at 0.1 Hz = 1440 samples).
- CPU: < 1% at 0.1 Hz.

See ADR-001.
