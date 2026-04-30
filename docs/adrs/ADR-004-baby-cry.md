# ADR-004: Baby cry detection cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `baby-cry`

## Context

Baby cry is a high-value detection for nursery / monitoring deployments.
The acoustic signature is **sustained mid-frequency energy** (typical
fundamental 300–600 Hz, harmonics through 4 kHz) lasting 1–10 s.

This is distinct from `cough-detect` (transient + spectral) — baby cry is
characterized by *duration*, not impulse.

## Decision

`baby-cry` runs at 5 Hz over the feature stream and tracks:

1. **Mid-band energy** — second-half feature channels weighted heavier than
   first-half (matching the cry harmonic distribution).
2. **Sustained-elevated counter** — frames-in-a-row where mid-band energy
   exceeds baseline by `cry_z` z-scores.
3. **Fire** when sustained for `cry_min_secs` continuously.
4. **Cooldown** to prevent ringing.

## CLI

```
cog-baby-cry [--once] [--interval 1] [--cry-z 2.5] [--cry-min-secs 2]
             [--cooldown 15]
```

## Output

```json
{
  "status": "quiet|elevated|CRY_DETECTED|cooldown",
  "cry_detected": false,
  "sustained_secs": 0.0,
  "midband_z": 0.0,
  "total_cries": 0,
  "timestamp": 1730000000
}
```

## Consequences

### Positive
- Audio-only — no camera privacy concerns in nursery setting.
- Sustained-energy logic is robust against single bangs.

### Negative
- Cannot distinguish baby cry from sustained adult yelling without ML.
- False positives from vacuum cleaners, blenders.

## Alternatives considered
- **CNN classifier on mel-spectrograms.** Rejected for v1 — model size.
- **Raw amplitude threshold.** Rejected — insufficient discrimination.

## Resource budget
- Binary: < 350 KB armhf.
- RAM: < 1 MB.
- CPU: < 2% at 5 Hz.

See ADR-001.
