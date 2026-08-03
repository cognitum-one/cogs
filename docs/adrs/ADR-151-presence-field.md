# ADR-151: Multi-node field-model presence cog

**Status**: Accepted — shipped as `presence-field` v0.1.1 (public registry,
difficulty `medium`). The breathing-band detector this ADR named as the exit
criterion **has landed, but ships advisory-only and does not gate presence**, so
still-person presence is still unproven. See Limitations.
**Date**: 2026-06-15
**Updated**: 2026-08-03
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
4. **Threshold + quorum + hold** — a node counts when its residual exceeds
   `thresh` × its empty-room floor; presence fires when at least `quorum`
   nodes count, and latches for `hold` seconds. `quorum` defaults to 1
   (equivalent to max-over-nodes); `quorum 2` is the N-of-M gate that rejects
   a single through-wall node.
5. **Calibration-quality guard** — after computing a baseline, the same
   breathing-band scan is run over the *calibration* frames. A stable in-band
   respiration peak means the room was not empty, and the operator is warned
   to re-calibrate. See Limitations: this is a warning, not a refusal.

**Port ownership changed after this ADR was written.** As drafted, the cog
owned UDP 5006 and relayed vitals/feature packets to a loopback port itself
(`--relay`, still present in the code). Under ADR-104 P2 the
`cognitum-csi-relay` is the single binder on :5006 and performs packet-type
fan-out, and the agent injects `COG_CSI_BIND=127.0.0.1:<per-cog port>` so this
cog is a relay *consumer*. Bind precedence is `--bind` > `COG_CSI_BIND` >
`0.0.0.0:5006`; the default is retained for standalone/manual runs only. The
per-cog port is a deterministic hash of the cog id — `presence-field` is always
5259. The cog's own `--relay` fan-out is therefore redundant in the deployed
path, kept for standalone use.

Health Monitor consumes `presence_detected` from this cog (via
`--presence-file`) and keeps its vitals gated behind it.

## CLI

```
cog-presence-field [--bind 0.0.0.0:5006] [--relay 127.0.0.1:5106]
                   [--calibrate <secs>] [--baseline <path>] [--presence-file <path>]
                   [--thresh 4.0] [--quorum 1] [--modes 8] [--hold 5] [--interval 1]
                   [--window 20]
                   [--breath-secs 45] [--breath-snr 4.0] [--breath-stable 3]
```

`--calibrate` runs the learning window and then **falls through into
detection** in the same process; operators are expected to stop it once
`baseline saved` prints.

`--window` is a count of **frames** to average per node, not milliseconds as
this ADR originally documented.

The `--breath-*` knobs and `--window` are **not** exposed in `cog.toml`, so
they are unreachable through the seed API — CLI only.

## Output

```json
{
  "presence_detected": false,
  "score": 1.7,
  "threshold": 4.0,
  "nodes_over_thresh": 0,
  "quorum": 1,
  "per_node_ratio": {"node1": 1.7},
  "breathing_advisory": false,
  "breathing_bpm": 0.0,
  "breathing_snr": 0.0,
  "nodes": 1,
  "method": "none",
  "timestamp": 1785460879
}
```

`method` is `field-residual` when the residual/quorum test fired this cycle,
`hold` while latched, `none` otherwise. `score` is the best per-node residual
ratio. The `breathing_*` fields are **advisory** — see Limitations.

## Limitations

- **The breathing-band detector landed, but it does not gate presence.** The
  work this ADR named as its exit criterion is implemented (`breathing_band()`,
  a Goertzel scan of the 0.10-0.50 Hz band over the per-node residual history,
  gated on SNR over the band's median floor plus `--breath-stable` consecutive
  consistent estimates). It is wired as **advisory only** and deliberately
  never sets `presence_detected`. Reason, recorded in the implementation: on
  real data the *empty-room* respiration SNR can equal or exceed the
  still-person SNR, because WiFi-CSI passes through walls and picks up family
  or pets elsewhere in the home. An in-band peak alone therefore cannot
  establish an occupant **in this room**, only corroborate a residual lock.
  So the exit criterion as originally written is satisfied by the code but not
  by the outcome, and this ADR's question is still open.
- **Still-person detection remains unproven, and the evidence disagrees with
  itself.** Two measurements are on record from 2026-06-15 and they have never
  been reconciled:
  - this ADR's own device run on cognitum-8b40 — seated person ~1.2-1.9×,
    below the 4.0 default; 14/14 `present=false` after ~15 s of stillness;
  - the implementation's note — still person 10-68× the empty floor on every
    node, with **3 nodes and a clean empty calibration**.

  The most likely reconciliation is baseline quality and node count, not the
  algorithm, but that is a hypothesis and has not been tested. Treat
  still-person presence as unvalidated until a controlled run settles it.
  The design-partner proof this ADR asked for (cognitum-4e61, 3 nodes) has
  **not** been recorded anywhere.
- **A contaminated baseline is the dominant field failure mode.** `floor` is
  the mean *in-sample* residual of the calibration frames, and the projected-out
  eigenmodes are computed from those same frames. An occupant present during
  calibration is therefore absorbed into both the divisor and the subtracted
  subspace, and every later reading pins near 1.0 whether the room is occupied
  or not — bench-verified 2026-06-15 (still-person ratio collapsed ~6× → ~2×).
  Observed in the field 2026-07-31: a design partner's single-node run warned
  `empty baseline shows a respiration band (16 bpm, snr 8.2)`, the baseline was
  saved anyway, and a person *walking into the room* then peaked at score 4.6
  against a 4.0 threshold. The calibration-quality guard warns and continues;
  it should refuse, or the saved baseline should carry the contamination
  verdict so a consumer can detect it.
- **Single-node `hd_distance` is unusable** — the single-node report carries
  no real distance field (reads 0 always); the multi-node baseline is the
  intended path, not `hd_distance`.
- **Calibration is launch-time `--calibrate` only on shipped firmware.** A
  runtime trigger + drift recalibration is a follow-up (see ADR-030). A real
  `POST /api/v1/apps/{id}/calibrate` endpoint exists in seed `main`
  (seed#290, fixed in `e7e7489` / seed#294), but v0.24.0 publication is held
  and field devices run v0.23.x, so the CLI flag is still the only route in
  practice.
- **`thresh` cannot be set through the seed API.** It is declared
  `type = "number"` in `cog.toml`; the agent's argv builder handles
  `boolean | integer | float | string` and drops anything else silently, so a
  `PUT /apps/presence-field/config` returns 200 and changes nothing. The other
  four keys (`quorum`, `modes`, `hold`, `interval`) are integers and do apply —
  except that a value equal to the declared default also emits no argv. Fixed
  in seed `main` under the same held release (seed#289). Until it ships, per-site
  threshold tuning — which this ADR's own config description calls for — is
  CLI-only.

## Consequences

- Health Monitor's safety gate is preserved end-to-end: no vitals are shown
  while `presence_detected` is false.
- Presence fires reliably on **motion and arrival**. That is enough for the
  building/occupancy use cases, and it is what this cog should be described as
  delivering today.
- The Vitality Call deployment is **not** unblocked. It gates all vitals behind
  `presence_detected` for a patient who rests still, which is precisely the
  case that remains unproven. Landing the breathing-band detector did not
  change this, because the detector is advisory.
- Store-only deployment of this cog still requires out-of-band SSH — tracked as
  cogs#38, with the fail-honest source/calibration contract as cogs#75.

## Follow-ups

- Settle still-person detection with a controlled run: clean calibration
  (verified no contamination warning), 1 node vs 3 nodes, seated subject,
  recorded ratios. Until then neither the 1.2-1.9× nor the 10-68× figure
  should be quoted as the behaviour of this cog.
- Make the calibration-quality guard fail closed, or persist the verdict into
  `baseline.json` so a running cog can report `calibration_suspect`.
- Re-declare `thresh` as `float`, or land seed#289 — whichever ships first.
- Expose `--window` and the `--breath-*` knobs in `cog.toml` if they are meant
  to be tunable per site; today they are unreachable through the API.
