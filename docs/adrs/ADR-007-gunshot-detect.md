# ADR-007: Gunshot detection cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `gunshot-detect`

## Context

Acoustic gunshot detection is a public-safety capability widely
deployed in stadiums, schools, and urban systems (ShotSpotter et al.).
The acoustic signature has three phases:

1. **Muzzle blast**: very high amplitude impulse, < 5 ms rise.
2. **Shock wave** (if supersonic round): brief secondary peak.
3. **Reverberation tail**: 100–300 ms decay.

This is distinguished from `glass-break` and other impulse cogs by
**peak amplitude** (gunshots saturate the input) and **decay shape**
(exponential, longer than impulse noise).

## Decision

`gunshot-detect` runs at 20 Hz (50 ms frames) and looks for:

1. **Saturating peak**: max amplitude > `peak_threshold` (default 0.95
   on normalized [-1,1] scale, i.e. very near saturation).
2. **Exponential decay**: post-peak frames show < 1.0× peak energy
   declining linearly in log-space.
3. **Cooldown**: prevents echo re-fires.

A `--ruview-mode` flag reinforces with CSI motion drop (people fleeing).

## CLI

```
cog-gunshot-detect [--once] [--interval 1] [--peak-threshold 0.95]
                   [--decay-frames 4] [--cooldown 30] [--ruview-mode]
```

## Output

```json
{
  "status": "quiet|peak|GUNSHOT|cooldown",
  "gunshot_detected": false,
  "peak_amplitude": 0.0,
  "decay_score": 0.0,
  "ruview_motion_drop": false,
  "total_shots": 0,
  "timestamp": 1730000000
}
```

## Consequences

### Positive
- Saturating-peak gate is robust against ordinary noise.
- 20 Hz captures decay shape.
- RuView mode adds an orthogonal evidence stream (post-event CSI motion).

### Negative
- A clipping audio system may produce false positives from any loud
  impulse — must tune `peak_threshold` per deployment.
- Cannot distinguish caliber.

## Alternatives considered
- **Multi-mic triangulation**. Rejected v1 — needs 3+ seeds in mesh.
  Possible v2 add-on.
- **CNN classifier**. Rejected v1 — model size; well-known false-positive
  failure modes anyway.

## RuView mode

When `--ruview-mode` and CSI features are available, the cog watches
for a sharp drop in CSI variance in the 5 s post-peak window
(interpreted as people freezing or fleeing). Each evidence stream
raises confidence by 25 percentage points.

## Resource budget
- Binary: < 380 KB armhf.
- RAM: < 1 MB.
- CPU: < 3% at 1 Hz default.

See ADR-001.
