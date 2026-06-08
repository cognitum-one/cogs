# cog-doom — DOOM on your Cognitum Seed

Play **DOOM** in your phone's browser, rendered on-device. The cog runs the
portable [`doomgeneric`](https://github.com/ozkl/doomgeneric) engine (software
rendered — no GPU, X11, or SDL) on a single engine thread and serves a
touch-friendly game client over a loopback HTTP API. The Seed agent
reverse-proxies `https://seed:8443/api/v1/cogs/doom/<path>` →
`http://127.0.0.1:8066/<path>`, injecting a per-cog bearer token.

> **First C-FFI cog.** Unlike the 100+ pure-Rust DSP cogs, this one compiles a
> vendored C engine (`build.rs` + the `cc` crate) and drives it through `extern
> "C"` callbacks. See [ADR-019](../../../docs/adrs/ADR-019-doom.md).

## Licensing — GPLv2 (read this)

**This cog is GPLv2, not MIT** like the rest of `cognitum-one/cogs`. It
statically links the vendored `doomgeneric` engine, which derives from id
Software's GPLv2 DOOM source — so the cog binary is a GPLv2 derivative. See
[`LICENSE`](LICENSE) (full GPLv2 text), [`NOTICE`](NOTICE), and
[`vendor/doomgeneric/README`](vendor/doomgeneric/README). Only this directory is
GPLv2; the rest of the repo stays MIT.

## Game data — FreeDoom (no DOOM WAD shipped)

The cog ships **no DOOM game data**. It uses the freely-redistributable
**FreeDoom** IWAD (`freedoom1.wad`) as a runtime asset:

* **FreeDoom v0.13.0**, `freedoom1.wad`
* sha256 `7323bcc168c5a45ff10749b339960e98314740a734c30d4b9f3337001f9e703d`
* 28,795,076 bytes, magic `IWAD`
* Source: <https://github.com/freedoom/freedoom/releases/download/v0.13.0/freedoom-0.13.0.zip>

It is declared in `cog.toml [[assets]]`; the agent fetches + sha256-verifies it
at install time and places it next to the binary. The WAD is **not** committed to
this repo. Maintainers may re-host it to the cognitum registry on publish (add a
`gcs_path` to the asset entry, as the other asset-bearing cogs do).

The cog resolves the WAD in this order:

1. `$DOOM_WAD` (explicit path)
2. `<exe_dir>/freedoom1.wad` (asset placed next to the binary)
3. `./freedoom1.wad` (current working dir)

If none exist it prints a clear error and exits. Any DOOM-format IWAD works
(e.g. a `doom1.wad` you own) via `DOOM_WAD=/path/to/your.wad`.

## HTTP API (paths after the proxy strips the prefix)

| Method | Path        | Auth   | Description |
|--------|-------------|--------|-------------|
| GET    | `/`         | open   | Mobile/desktop game client (`assets/index.html`). |
| GET    | `/health`   | open   | `{"status":"ok"}`. |
| GET    | `/frame`    | paired | JPEG of the latest frame. `?since=N` long-polls up to 1 s for a frame newer than N; returns `204` on timeout. `X-Frame-Id` carries the frame counter. |
| POST   | `/input`    | paired | `{"events":[{"action":"down"|"up","key":"<name>"}]}` (also accepts a bare array or single object). |
| GET    | `/stream`   | paired | `multipart/x-mixed-replace` MJPEG loop (best-effort). |

Key names: `left right up down fire use enter escape tab strafeleft straferight
run space y n 1 2 3 4 5 6 7`.

## Controls

* **Touch:** on-screen D-pad, FIRE/USE, RUN toggle, weapon `1–7`, ESC/ENTER/MAP/SPC.
* **Keyboard:** `W/S` move · `←/→` turn · `A/D` strafe · `Space` fire · `E` use ·
  `Shift` run · `1–7` weapons · `Tab` map · `Enter/Esc` menu. (Fire is Space and
  Use is E — not classic Ctrl-fire — so browser shortcuts like Ctrl+W/A/S/D keep
  working.)
* A **Scale** dropdown chooses on-screen size/filter (the engine always renders
  320×200; this is purely client-side).

## Auth model (ADR-095 default)

By default the game endpoints (`/frame`, `/input`, `/stream`) require
`Authorization: Bearer <COGNITUM_COG_TOKEN>` (constant-time compare via
`subtle`). `/` and `/health` are always open. The seed agent injects
`COGNITUM_COG_TOKEN` at `/start`. With no token and no open-mode, the game
endpoints return `401` for everything (safe default).

### Optional direct-LAN mode (off by default)

For playing directly over the LAN without the agent proxy (which is rate-limited
to ~1 fps), set **both**:

```sh
DOOM_OPEN=1        # disable the bearer-token check on the game endpoints
DOOM_BIND=0.0.0.0  # bind all interfaces instead of loopback
```

**Security tradeoff:** this makes the game's frame/input endpoints reachable by
**anyone on the local network**, bypassing the agent's auth + rate limiting. It
is a deliberate convenience option, **OFF by default**. Only the *game*
endpoints are affected — never the device/agent API. See ADR-019 for the
rationale and the ADR-095 tradeoff.

## Environment

| Var | Default | Purpose |
|-----|---------|---------|
| `COGNITUM_COG_PORT`   | `8066` | Bind port. |
| `COGNITUM_COG_TOKEN`  | unset  | Per-cog bearer token (required on game endpoints unless `DOOM_OPEN=1`). |
| `COGNITUM_DATA_DIR`   | `$TMP/cog-doom` | Writable dir for savegames/config; the engine `chdir`s here. |
| `DOOM_WAD`            | unset  | Explicit IWAD path (overrides the `<exe_dir>` / `./` lookup). |
| `DOOM_BIND`           | `127.0.0.1` | Bind address. `0.0.0.0` faces the LAN directly. |
| `DOOM_OPEN`           | unset  | `=1` disables token auth (direct-LAN play). |
| `DOOM_JPEG_QUALITY`   | `55`   | JPEG quality 1–100 (also `cog.toml [config.jpeg_quality]` / `--jpeg-quality`). |

## Build

### Native (x86_64 smoke test)

Place a WAD next to the binary first (or set `DOOM_WAD`):

```sh
cargo build --release
cp /path/to/freedoom1.wad target/release/freedoom1.wad
COGNITUM_COG_TOKEN=dev ./target/release/cog-doom    # binds 127.0.0.1:8066
# then: curl -H 'Authorization: Bearer dev' http://127.0.0.1:8066/frame -o f.jpg
```

A C compiler is required (`build.rs` compiles the vendored doomgeneric engine).

### ARM (Pi Zero 2 W / armhf) — the shippable artifact

```sh
docker build -t cog-doom-arm -f Dockerfile .
docker create --name x cog-doom-arm && docker cp x:/cog-doom-arm ./cog-doom-arm && docker rm x
# -> cog-doom-arm (ELF 32-bit ARM hard-float, dynamically linked glibc, stripped)
```

The Dockerfile sets BOTH the Rust linker
(`CARGO_TARGET_..._LINKER`) and the `cc`-crate cross-compiler
(`CC_armv7_unknown_linux_gnueabihf`) so the C engine cross-compiles too. The
build context is this directory — everything `build.rs` / `include_str!`
reference (`vendor/`, `assets/`) lives inside it.

## No audio

The engine is compiled against the generic sound stub (no SDL/Allegro mixer), so
there is **no sound** — only video + input. This keeps the binary dependency-free
(just libc + libm) and small. See ADR-019.
