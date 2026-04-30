# ADR-002: Fall detection cog

**Status**: Accepted
**Date**: 2026-04-29
**Cog**: `fall-detect`

## Context

Falls are the leading cause of injury for adults over 65 and a top OSHA
recordable across construction, warehousing, and food-service. The seed
already ships `gait-analysis` (biomarker trending) but has **no event
detector** — by the time gait drift surfaces, the fall has already
happened.

A useful fall detector on a Pi Zero 2 W needs to:

1. Distinguish a true fall (rapid acceleration spike followed by sustained
   stillness) from sit-downs, dropped objects, and door slams.
2. Run at low duty-cycle so a single seed can monitor multiple cogs
   simultaneously.
3. Not require a worn device — must work from ambient sensor data
   (microphone amplitude + ESP32 motion features).

## Decision

`fall-detect` runs at 5 Hz over the standard `cog-sensor-sources` feature
stream and uses a three-stage state machine:

1. **Quiet** — baseline motion variance below `quiet_threshold` (Welford
   running stats over 30 s window).
2. **Impact** — single-frame variance > `impact_threshold` AND mean
   amplitude > `impact_amplitude`. Captures the fall itself.
3. **Stillness** — 5+ consecutive frames at < 0.3× quiet baseline within
   `stillness_window_secs` of impact. Captures the post-fall lying-still.

Both Impact AND Stillness must occur to fire `FALL_DETECTED`. A single
loud bang (Impact only, no Stillness) is suppressed.

A cooldown of `cooldown_secs` after a fire prevents ringing.

### CLI

```
cog-fall-detect [--once] [--interval 1]
                [--impact-threshold 6.0] [--stillness-window 8]
                [--cooldown 30] [--ruview-mode]
```

### Output

```json
{
  "status": "quiet|impact|monitoring|FALL_DETECTED|cooldown",
  "fall_detected": false,
  "confidence": 0.0,
  "z_impact": 0.0,
  "stillness_pct": 0.0,
  "total_falls": 0,
  "timestamp": 1730000000
}
```

## Consequences

### Positive

- No wearable required — picks up on ambient feature stream.
- 5 Hz is achievable on Pi Zero 2 W; cog idles between samples.
- Two-stage gate (impact + stillness) cuts false positives from dropped
  objects, slammed doors, dog jumping off couch.

### Negative

- Misses a slow slide-to-floor fall (no impact spike).
- Cannot triangulate location with a single seed; need 2+ seeds in mesh
  for room-level accuracy.

### Neutral

- Confidence is heuristic, not calibrated. Tuning `impact_threshold` per
  deployment is expected.

## Alternatives considered

- **Wearable accelerometer broadcast.** Rejected for v1: requires user
  cooperation. Could be added as a secondary input later.
- **ML classifier on raw audio.** Rejected: model size + Pi Zero 2 W RAM
  budget. Possible v2 with a quantized TFLite model (~200 KB).
- **Single-stage threshold.** Rejected: too many false positives in
  household / workplace settings.

## RuView mode

When invoked with `--ruview-mode` and CSI features are present in the UDP
stream, `fall-detect` re-interprets the 8-feature frame as a coarse pose
descriptor (head height proxy from vertical CSI subcarriers). A sudden
drop in head-height proxy below 25% of baseline within 1 s reinforces the
impact stage and raises confidence by +20 percentage points.

This is **optional** — without ruview, the cog falls back to amplitude/
variance only. The feature flag is `--ruview-mode`, not a Cargo feature,
because the same binary handles both modes.

## Resource budget

- **Binary size**: target < 400 KB stripped armhf.
- **RAM**: < 2 MB resident (Welford stats + 30 s ring buffer).
- **CPU**: < 5% of one core at 5 Hz default.

See ADR-001 for the cog-as-plugin architecture this slots into.
