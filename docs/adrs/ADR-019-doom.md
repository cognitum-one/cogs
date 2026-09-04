# ADR-019: DOOM cog

**Status**: Proposed
**Date**: 2026-06-07
**Cog**: `doom`
**Related**: [ADR-001 (Cogs as plugins)](ADR-001-cogs-as-plugins-architecture.md), [cognitum-one/seed ADR-095 (Cogs as API Providers)](https://github.com/cognitum-one/seed/blob/main/docs/seed/ADR-095-cogs-as-api-providers.md)

## Context

Operators want a flagship "it just works" demo that shows the Seed is a real
computer, not only a sensor box: open the Seed dashboard, tap **DOOM**, and play
the 1993 game in your phone's browser — rendered entirely on the device, no
cloud, no app install.

This cog is structurally unlike the ~100 sensor → DSP → result cogs in this repo
in two ways:

1. **It is the first C-FFI cog.** Every other cog is pure Rust. DOOM is a large,
   battle-tested C codebase; reimplementing it in Rust is out of scope. We
   instead compile the portable [`doomgeneric`](https://github.com/ozkl/doomgeneric)
   engine (a thin platform-abstraction fork of id Software's GPLv2 DOOM source)
   and drive it over `extern "C"` callbacks.
2. **It is GPLv2, inside an MIT repo.** Linking GPLv2 DOOM source makes the cog
   binary a GPLv2 derivative. This is a licensing question the maintainers must
   sign off on (see *Licensing* below) — it is the most important thing in this
   ADR.

The infrastructure for cogs-as-API-providers (agent proxy, per-cog bearer
tokens, loopback bind, asset distribution) is defined in cognitum-one/seed
ADR-095. This ADR records the cog-side decisions that align with it.

## Decision

`doom` v0.1.0 ships as a binary, API-provider cog.

### Engine: vendored doomgeneric, compiled by build.rs

`build.rs` compiles the upstream `SRC_DOOM` object list via the `cc` crate, minus
every platform backend (`doomgeneric_sdl.c` / `_xlib.c` / `_allegro.c` / `_win.c`
/ `_soso*.c` / `_linuxvt.c` / `_emscripten.c`) and the SDL/Allegro sound mixers.
The cog provides its own **headless** `DG_*` backend in `src/main.rs`: one
dedicated engine thread owns all of doomgeneric (it is not thread-safe), calls
`doomgeneric_Create()` once, then loops `doomgeneric_Tick()`. The `DG_DrawFrame`
callback copies the XRGB framebuffer into a shared RGB buffer; `DG_GetKey` pops
from a shared input queue. HTTP worker threads only ever read the shared frame /
push input — they never touch the engine.

We **vendor** the needed C sources (`vendor/doomgeneric/`) rather than use a git
submodule, so the PR is self-contained and CI's `cargo check` builds without
`git submodule update`. The exact compiled source set is pinned and reviewable
in-tree.

**No audio**: compiled against the generic sound stub, so the binary needs only
libc + libm. Video + input only.

### Transport: polling JPEG-over-HTTP (not WebSocket / not streaming)

The engine renders at DOOM's native **320×200**, and each frame is JPEG-encoded
(quality 55 by default, operator-configurable). The browser client long-polls
`GET /frame?since=N` and posts input to `POST /input`.

Why not a WebSocket or a persistent MJPEG stream?

* **The agent proxy buffers streams and has no WS support.** The seed agent's
  `/api/v1/cogs/<id>/*` reverse proxy (ADR-095) is built for request/response
  JSON APIs. It buffers response bodies (so a never-ending MJPEG `/stream`
  stalls behind the buffer) and does not speak the WebSocket upgrade handshake.
  A polling JPEG API is the transport that survives the proxy unchanged.
* **Content-Length, never chunked.** The proxy leaks chunk framing into the body
  and relabels `image/jpeg` as `application/octet-stream`. The cog forces
  `Content-Length` on `/frame`; the client re-wraps the bytes as an
  `image/jpeg` Blob. (`/stream` MJPEG exists for direct-LAN browsers but is
  best-effort.)
* **320×200 for encode speed.** JPEG encoding is the framerate bottleneck on a Pi
  Zero 2 W. Native 320×200 is ~1/4 the pixels of 640×400, making each encode
  ~3–4× cheaper and frames far smaller; the phone upscales (classic chunky DOOM).
* The client uses **adaptive frame pacing**: it backs off on HTTP 429 and speeds
  up while frames flow, self-tuning to whatever rate tier the agent grants.

### Loopback-only + bearer-token auth (ADR-095 default)

`cog.toml [api] bind_loopback_only = true`; the cog binds `127.0.0.1:8066`. The
game endpoints (`/frame`, `/input`, `/stream`) require the per-cog bearer token
(constant-time compare via `subtle`); `/` and `/health` are open. With no token
injected and open-mode off, the game endpoints `401` everything — the safe
default. The agent injects `COGNITUM_COG_TOKEN` at `/start`.

| Method | Path | Auth | Purpose |
|---|---|---|---|
| GET  | `/`       | open   | Browser game client |
| GET  | `/health` | open   | Liveness |
| GET  | `/frame`  | paired | Latest frame as JPEG (`?since=` long-poll) |
| POST | `/input`  | paired | Key down/up events |
| GET  | `/stream` | paired | MJPEG loop (best-effort, for direct-LAN) |

### Optional direct-LAN mode (off by default)

Two env flags enable playing directly over the LAN, bypassing the agent proxy
(and its ~1 fps rate limit): `DOOM_OPEN=1` disables the token check on the game
endpoints, and `DOOM_BIND=0.0.0.0` binds all interfaces. **Both off by default.**

**ADR-095 tradeoff:** ADR-095's posture is loopback-only + token-gated. Direct-LAN
mode deliberately relaxes that for a home-game use case: the *game* frame/input
endpoints become reachable by anyone on the LAN, with no auth and no agent rate
limiting. It never exposes the device/agent API — only this cog's game endpoints.
This is opt-in, loudly logged at startup, and documented in the README. If
maintainers prefer, the flags can be dropped entirely and the cog left
proxy-only; nothing else depends on them.

### Game data: FreeDoom asset (ADR-095 §4)

The cog ships **no DOOM game data**. It uses the freely-redistributable
**FreeDoom** IWAD as a runtime asset, declared in `cog.toml [[assets]]`:

* FreeDoom **v0.13.0**, `freedoom1.wad`
* sha256 `7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d`,
  28,795,076 bytes.
* Source: the FreeDoom GitHub release `freedoom-0.13.0.zip`.

The agent fetches + sha256-verifies it at install and places it next to the
binary; the cog resolves `$DOOM_WAD` → `<exe_dir>/freedoom1.wad` → `./freedoom1.wad`.
The WAD is **not** committed (it's an asset). Maintainers may re-host it to the
cognitum registry on publish (add `gcs_path` to the asset entry). FreeDoom is its
own project under a BSD-3-Clause license and is unrelated to the GPLv2 engine
code below.

### Naming convention (CI consistency)

* `Cargo.toml [[bin]] name = "cog-doom"` — what `cargo build` outputs and what
  `ci.yml :: manifest-validate` requires (`cog-<dirname>`).
* `cog.toml binary = "cog-doom-arm"` — the cross-compiled + stripped armhf
  artifact published to the registry, matching every other cog's `-arm` suffix.

## Licensing (GPLv2 vs the repo's MIT — please read)

**This cog directory is GPLv2; the rest of `cognitum-one/cogs` is MIT.** Linking
the vendored doomgeneric engine (derived from id Software's GPLv2 DOOM source)
makes the `doom` cog binary a GPLv2 derivative work. We therefore set
`Cargo.toml license = "GPL-2.0-or-later"` and ship:

* `src/cogs/doom/LICENSE` — the full GPLv2 text governing this cog.
* `src/cogs/doom/NOTICE` — explains the GPLv2-in-an-MIT-repo situation and its
  scope (only this directory).
* `vendor/doomgeneric/LICENSE` + `vendor/doomgeneric/README` — the engine's own
  GPLv2 license and origin.

GPL applies to this self-contained derivative work; it does not relicense the
repo. **Maintainers must confirm they accept a GPLv2 subdirectory inside this
otherwise-MIT repo.** If that is not acceptable, the alternative is to host the
doom cog in a separate repository referenced from the registry — but vendoring
keeps the PR self-contained and CI green. This is the open decision for review.

## Consequences

- **Positive**: a flagship, genuinely fun demo; proves the API-provider +
  asset-distribution path end-to-end; establishes the first C-FFI + build.rs
  pattern other ports (other games, codecs) can reuse.
- **Negative**: GPLv2 obligation lives in the repo; first cog with a C toolchain
  dependency in CI (the runner already has `cc`, so `cargo check` works); larger
  vendored source tree (~2 MB of C).
- **Neutral**: no audio in v0.1.0; performance on a real Pi Zero 2 W
  (software render + per-frame JPEG at 320×200) is expected to be playable but is
  not yet field-measured.

## Alternatives considered

- **WebSocket frame transport** — rejected: the agent proxy has no WS support;
  would only work in direct-LAN mode, splitting the client in two.
- **Persistent MJPEG `/stream` as the primary path** — rejected: the proxy
  buffers streamed bodies. Kept as a best-effort secondary for direct-LAN.
- **640×400 rendering** — rejected: JPEG encode cost ~4× higher on the Pi Zero;
  320×200 upscaled in the browser looks correct for DOOM.
- **git submodule for doomgeneric** — rejected: CI `cargo check` would need
  `submodule update`; vendoring keeps the PR self-contained and pins the exact
  compiled sources.
- **Embedding the WAD via `include_bytes!`** — rejected: bloats the binary, and
  shipping game data in-repo is the wrong distribution channel. The asset path
  (ADR-095 §4) is the right mechanism; FreeDoom is the redistributable IWAD.
- **Separate GPL repo** — viable fallback if maintainers reject GPL-in-repo; see
  Licensing.

## RuView mode (optional)

Not applicable. `doom` is an interactive game cog; it consumes no sensor stream
and does not integrate with ruvnet/ruview WiFi-CSI input.
