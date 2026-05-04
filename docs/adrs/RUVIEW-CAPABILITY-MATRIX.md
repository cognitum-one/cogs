# RuView Capability Matrix

How the 15 new cogs (ADR-002 through ADR-016) integrate with the
ruvnet/ruview ESP32 WiFi-CSI feature stream.

## Three integration modes

| Mode         | Behavior                                                                        |
| ------------ | ------------------------------------------------------------------------------- |
| **none**     | Cog ignores CSI specifics; uses raw amplitudes and variances only.              |
| **optional** | Cog has a `--ruview-mode` flag. When set, adds CSI-derived signals as evidence. Without the flag, falls back to non-CSI inputs. |
| **required** | Cog refuses to run without CSI input. Uses CSI subcarrier-amplitude shifts as primary signal. |

## Per-cog matrix

| ADR  | Cog                       | Mode      | What CSI adds (when used)                                                |
| ---- | ------------------------- | --------- | ------------------------------------------------------------------------ |
| 002  | `fall-detect`             | optional  | Head-height proxy (drop > 75% reinforces impact stage; +20% confidence)  |
| 003  | `cough-detect`            | none      | -                                                                        |
| 004  | `baby-cry`                | none      | -                                                                        |
| 005  | `snore-monitor`           | none      | -                                                                        |
| 006  | `glass-break`             | none      | -                                                                        |
| 007  | `gunshot-detect`          | optional  | Post-peak CSI variance drop (people freezing) raises confidence +25%     |
| 008  | `package-detect`          | required  | Subcarrier-shift = primary signal for static-object presence            |
| 009  | `ppe-compliance`          | required* | Reads `ruview-densepose` skeleton output via store/search composition   |
| 010  | `slip-fall-zone`          | optional  | Cautious-gait CSI score (slow step rate proxy) joins risk fusion        |
| 011  | `water-leak`              | none      | -                                                                        |
| 012  | `smoke-fire`              | optional  | Plume signature (ratio of recent vs. early variance) as 3rd signal      |
| 013  | `frost-warning`           | none      | -                                                                        |
| 014  | `beehive-monitor`         | none      | -                                                                        |
| 015  | `predictive-maintenance`  | none      | -                                                                        |
| 016  | `parking-occupancy`       | required  | Per-zone subcarrier-amplitude is primary signal                         |

`*` `ppe-compliance` requires another cog (`ruview-densepose`) to be running upstream.
It composes via the seed RuVector store rather than reading CSI directly.

## How RuView gets to a cog

The path is:

```
ESP32 (RuView firmware v0.6.3+)
    │  WiFi-CSI features extracted onboard
    │  packed into ADR-069 MAGIC_FEATURES UDP packet (8 LE-f32 features)
    ▼
seed agent (LAN UDP listener on :5006)
    │  re-packs into /api/v1/sensor/stream JSON shape
    ▼
cog (calls cog_sensor_sources::fetch_sensors())
    │  reads `samples[].value` array
    ▼
cog interpretation:
  - non-ruview cogs: treat as raw amplitudes
  - ruview cogs: interpret as 8 subcarrier amplitudes / pose proxies
```

The same UDP magic packet feeds all cogs. Whether a cog "uses ruview"
is purely a matter of how it interprets the 8 features — there's no
separate UDP channel.

## Detecting RuView at runtime

A ruview-required cog can detect whether the live feature stream is
ruview-shape by checking `sensors["samples"][0]["sensor"]` in the
`fetch_sensors()` response — the `cog-sensor-sources` crate tags
`"esp32-udp"` for ADR-069 packets and `"synthetic"` for fallback /
loopback. See `crates/cog-sensor-sources/src/lib.rs`.

## How to enable RuView mode

Cogs with `optional` mode:

```
cog-fall-detect --ruview-mode --once
cog-gunshot-detect --ruview-mode --once
cog-slip-fall-zone --ruview-mode --once
cog-smoke-fire --ruview-mode --once
```

Cogs with `required` mode (fail gracefully if no CSI):

```
cog-package-detect --once
cog-parking-occupancy --once
cog-ppe-compliance --once  # needs ruview-densepose installed and running
```

## ESP32 firmware requirements

RuView features (head-height proxy, subcarrier amplitudes, motion
vectors) need ESP32 firmware **v0.6.3-esp32** or later. Older firmwares
emit raw audio amplitudes only, in which case `--ruview-mode` flags
silently degrade to non-ruview behavior.

Cogs do not detect ESP32 firmware version; users must verify.
