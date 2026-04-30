# ADR-014: Beehive monitor cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `beehive-monitor`

## Context

Hive health is tracked acoustically by experienced beekeepers — the
sound of a healthy hive is steady (~250 Hz hum), while:

- **Queenless** hives produce a louder, more chaotic, higher-pitched
  buzz (300–400 Hz, broader spectrum).
- **Swarming preparation** produces a distinct "piping" sound from
  virgin queens (~500 Hz pulses).
- **Robbing** events are louder + spike around 200 Hz.

A seed near a hive with the ESP32 audio mic can track these signatures
and report hive state.

## Decision

`beehive-monitor` runs at 0.1 Hz (every 10 s — bees don't need higher
sampling) and tracks:

1. **Hum-band energy** — first-half feature channels.
2. **Chaos score** — variance of recent absolute amplitudes.
3. **Piping detector** — autocorrelation peak in 0.3-1 Hz pulse rate.
4. **Robbing detector** — sharp upward energy excursion vs. baseline.

Outputs hive state classification.

## CLI

```
cog-beehive-monitor [--once] [--interval 10]
                    [--chaos-z 1.5] [--robbing-z 3.0]
```

## Output

```json
{
  "status": "healthy|chaotic|QUEENLESS|SWARMING|ROBBING",
  "queen_loss_likely": false,
  "swarming_likely": false,
  "robbing_likely": false,
  "hum_energy": 0.0,
  "chaos_z": 0.0,
  "piping_rate_hz": 0.0,
  "timestamp": 1730000000
}
```

## Consequences

### Positive
- Niche but high-value for beekeepers.
- Acoustic-only — no hive penetration / disturbance.
- Slow sampling rate is good for solar-powered field deployment.

### Negative
- Cannot diagnose specific diseases (varroa, foulbrood) — only
  acoustically-visible state.

## Alternatives considered
- **Hive scale weight tracking**. Out of scope — separate hardware.
- **Internal temperature sensor**. Out of scope — separate hardware.

## Resource budget
- Binary: < 360 KB armhf.
- RAM: < 1.5 MB.
- CPU: < 1% at 0.1 Hz.

See ADR-001.
