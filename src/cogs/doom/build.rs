// Compile the vendored doomgeneric C engine into this crate.
//
// We mirror the upstream Makefile's SRC_DOOM object list (the proven-good compile
// set) but DROP every platform backend (doomgeneric_sdl.c / _xlib.c / _allegro.c /
// _win.c / _soso*.c / _linuxvt.c / _emscripten.c) — the DG_* platform callbacks
// are provided by our Rust code in src/main.rs instead. We also never include
// mus2mid (its own STANDALONE main) or the SDL/Allegro sound mixers (external libs).
//
// The C sources live IN this crate under vendor/doomgeneric/doomgeneric so the PR
// is self-contained and CI builds without git submodules. doomgeneric is GPLv2
// (see vendor/doomgeneric/LICENSE) — linking it makes this cog a GPLv2 derivative.

use std::path::Path;

fn main() {
    let dg = "vendor/doomgeneric/doomgeneric";
    let dgp = Path::new(dg);
    assert!(
        dgp.join("doomgeneric.c").exists(),
        "doomgeneric sources not found at {dg} (expected vendor/doomgeneric/doomgeneric)"
    );

    // This is the upstream Makefile SRC_DOOM list with the platform backends removed.
    // i_sound.c here is the generic stub layer (no SDL/Allegro mixer) which the
    // engine compiles against fine; no actual audio output, which is what we want.
    let files = [
        "dummy.c",
        "am_map.c",
        "doomdef.c",
        "doomstat.c",
        "dstrings.c",
        "d_event.c",
        "d_items.c",
        "d_iwad.c",
        "d_loop.c",
        "d_main.c",
        "d_mode.c",
        "d_net.c",
        "f_finale.c",
        "f_wipe.c",
        "g_game.c",
        "hu_lib.c",
        "hu_stuff.c",
        "info.c",
        "i_cdmus.c",
        "i_endoom.c",
        "i_joystick.c",
        "i_scale.c",
        "i_sound.c",
        "i_system.c",
        "i_timer.c",
        "memio.c",
        "m_argv.c",
        "m_bbox.c",
        "m_cheat.c",
        "m_config.c",
        "m_controls.c",
        "m_fixed.c",
        "m_menu.c",
        "m_misc.c",
        "m_random.c",
        "p_ceilng.c",
        "p_doors.c",
        "p_enemy.c",
        "p_floor.c",
        "p_inter.c",
        "p_lights.c",
        "p_map.c",
        "p_maputl.c",
        "p_mobj.c",
        "p_plats.c",
        "p_pspr.c",
        "p_saveg.c",
        "p_setup.c",
        "p_sight.c",
        "p_spec.c",
        "p_switch.c",
        "p_telept.c",
        "p_tick.c",
        "p_user.c",
        "r_bsp.c",
        "r_data.c",
        "r_draw.c",
        "r_main.c",
        "r_plane.c",
        "r_segs.c",
        "r_sky.c",
        "r_things.c",
        "sha1.c",
        "sounds.c",
        "statdump.c",
        "st_lib.c",
        "st_stuff.c",
        "s_sound.c",
        "tables.c",
        "v_video.c",
        "wi_stuff.c",
        "w_checksum.c",
        "w_file.c",
        "w_main.c",
        "w_wad.c",
        "z_zone.c",
        "w_file_stdc.c",
        "i_input.c",
        "i_video.c",
        "doomgeneric.c",
        // NOTE: no doomgeneric_*.c platform backend — Rust provides the DG_* backend.
    ];

    let mut build = cc::Build::new();
    build
        .include(dg)
        .warnings(false)
        .flag_if_supported("-w")
        .flag_if_supported("-fno-strict-aliasing")
        .define("NORMALUNIX", None)
        .define("LINUX", None)
        .define("_DEFAULT_SOURCE", None)
        // Render at DOOM's native 320x200 (must match RESX/RESY in src/main.rs).
        .define("DOOMGENERIC_RESX", "320")
        .define("DOOMGENERIC_RESY", "200")
        .opt_level_str("s");

    for f in files {
        let p = dgp.join(f);
        assert!(p.exists(), "missing doomgeneric source: {}", p.display());
        build.file(p);
    }

    build.compile("doomgeneric");

    println!("cargo:rustc-link-lib=m");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={dg}");
}
