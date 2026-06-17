# ADR-151: Multi-node field-model presence cog

**Status**: Proposed (WIP — still-person detection pending, see Limitations)
**Date**: 2026-06-15
**Cog**: `presence-field`

## Context

Single-node WiFi-CSI cannot reliably tell an empty room from an occupied
one when the person is **still**: phase/amplitude variance from a
motionless body settles into a low-variation state that fades to — or
below — the empty-room floor. So a single-node presence flag only fires on
motion or arrival, and drops a quietly resting occupant to `present=false`.

This blocks medical monitoring. The Vitality Call pilot (design partner)
gates **all** vitals behind `presence_detected` — a real heart rate /
breathing reading is never displayed for an unconfirmed person — and their
patient rests still for long stretches. Real contactless vitals already
flow (HR 40-50 bpm, resp 8-21) on `auto:esp32-vitals`, but stay hidden
whenever presence fails to lock. Lowering the threshold is not an option —
it surfaces vitals for an empty room.

## Decision

`presence-field` is a multi-node presence cog using a **field-model
residual** across ≥2 CSI nodes:

1. **Per-node empty-room baseline** — learned during a calibration window
   (room left empty), persisted to disk.
2. **Project out environmental modes** — the top-`modes` eigenmodes of the
   per-node signal (static multipath / environment) are removed.
3. **Residual = body perturbation** — what remains after projection is the
   occupant's effect on the field; presence energy is the residual maxed
   across nodes.
4. **Threshold + hold** — a person is present when the residual exceeds
   `thresh` × the empty-room floor; presence latches for `hold` seconds.

The cog **owns UDP 5006** (the ESP32 ingest port) and **relays** the
vitals/feature packets to a loopback port, so a vitals cog (Health
Monitor) can coexist on the same node feed without contending for :5006.
Health Monitor consumes `presence_detected` from this cog and keeps its
vitals gated behind it.

## CLI

```
cog-presence-field [--bind 0.0.0.0:5006] [--relay 127.0.0.1:5007]
                   [--calibrate] [--baseline <path>] [--presence-file <path>]
                   [--thresh 4.0] [--modes 8] [--hold 5] [--interval 1]
                   [--window <ms>]
```

## Output

```json
{
  "presence_detected": false,
  "presence_score": 1.7,
  "node_count": 2,
  "thresh": 4.0,
  "source": "field-model-residual"
}
```

## Limitations (why this is still WIP / draft)

- **Still-person detection is not solved yet.** The residual as computed is
  a *motion* measure; a motionless body still fades toward the empty
  baseline, so a person holding still reads below threshold (device-verified
  on cognitum-8b40: seated person ~1.2-1.9×, below the 4.0 default; 14/14
  reads `present=false` after ~15 s of stillness). Robust still-person
  presence needs a **breathing-band detector** (isolate the ~0.1-0.5 Hz
  respiration modulation as the liveness signal) — tracked as the remaining
  work before this cog leaves draft.
- **Single-node `hd_distance` is unusable** — the single-node report carries
  no real distance field (reads 0 always); the multi-node baseline is the
  intended path, not `hd_distance`.
- **Calibration is launch-time `--calibrate` only.** A runtime
  trigger + drift recalibration is a follow-up (see ADR-030).

## Consequences

- Health Monitor's safety gate is preserved end-to-end: no vitals are shown
  while `presence_detected` is false.
- Unblocks the Vitality Call deployment **once** the breathing-band
  detector lands and still-person presence is proven on their unit
  (cognitum-4e61, 3 nodes), not just the cognitum-8b40 test seed.
