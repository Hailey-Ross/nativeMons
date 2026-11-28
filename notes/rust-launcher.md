# Rust launcher

`launcher-rs/`: the launcher rebuilt in Rust, so one program runs natively on Windows and on Linux and the Steam Deck, and costs less on low-end PCs. Not released yet; the C# launcher (`launcher/NativeMonsLauncher.cs`, [launcher.md](launcher.md)) is the one that ships.

The rule for the rebuild: every screen, icon, layout, color and animation stays identical to the C# launcher. Text rendering is the only allowed difference. Everything is drawn in code, as in the C#, and parity is checked picture by picture against the C# launcher's own output.

Building it: [docs/building.md](../docs/building.md#the-rust-launcher-not-released-yet).

## Crates

| Crate | Holds |
| --- | --- |
| `nm-core` | Everything without a window: config, the library and ROM scan, saves (SaveGuard, save sets, sharing between engines), mods, PC games, updates, 3DS builds. One module per C# static class, with the same names. |
| `nm-draw` | A `System.Drawing`-shaped API (`Graphics`, `GraphicsPath`, `Pen`, `Brush`, `Bitmap`, `StringFormat`) that reproduces GDI+'s pixels, over tiny-skia and cosmic-text. Also the code-drawn icons and .NET's `Random`. |
| `nm-ui` | The C# `LauncherForm` without the platform: pages, rendering, animations, input as a state machine, hit-testing, the job model and the play flows. |
| `nm-platform` | The parts that differ per system: the SDL3 window, presenting, controllers, one launcher per folder, requests from a second start, Light mode's detection. |
| `nativemons` | The program: the command-line modes, then the live loop. |
| `golden-diff` | Development only: compares a Rust picture with the C# one (see Parity). |

The port is by hand and line by line: C# names, order and `float`/`double` types are kept, so a render method in `nm-ui` reads like its C# original and takes a `&mut Graphics` the same way. Where a name had to change, a short `// C#: Name` says what it was.

## Drawing

GDI+ isn't available on Linux, and a stock 2D library draws edges, gradients and text a little differently. So `nm-draw` is a clone of what GDI+ does, worked out by measuring it: `tools/gdiplus-probe` is a small C# tool (never shipped) that runs drawing scripts through GDI+ with the launcher's settings and writes the raw pixels, metrics and flattened paths. The same scripts run through `nm-draw`, and the results are diffed. That gave the coverage grid, curve flattening, pen widening, dash patterns, clip behavior, gradient wrap and rounding, image scaling and .NET's random numbers, most of them bit for bit.

- **tiny-skia** underneath: CPU only, so it draws the same with no window, in tests and on every system.
- **Text:** cosmic-text and swash with bundled fonts only, never the system's: the Noto Sans set the C# launcher ships (`launcher/fonts`), plus Noto Sans Symbols 2 (`launcher-rs/fonts`) for characters Noto Sans lacks, such as ▶ ◀ ★, the way GDI+ links fonts on Windows. Measuring follows GDI+'s, so layout around text lands in the same place.
- **Icons:** the C# launcher draws its icons from the Segoe MDL2 Assets font, which Linux doesn't have. Here each one is a set of paths in `nm_draw::icons`, measured from GDI+'s renders of the glyph and compared at large and small sizes.
- **Speed:** the live loop records each frame's drawing calls and redraws only the areas where they differ from the last frame, so a still menu costs almost nothing. A full frame of the home screen takes about 15 ms in a release build, an idle one about 3 ms. The pixels are the same as a full redraw; the parity runs check that too.

## Light mode

Rust only. For low-end PCs and handhelds: the same screens, icons and transitions, at most 30 frames a second, and a 3DS background build that's already running is paused (not canceled) while a game plays, then resumes. It's chosen automatically (few processor threads, little memory, or a Steam Deck or Wine on battery), and Settings has Auto, On and Off. The choice is kept in `state\light-mode.txt`.

## Parity

The C# launcher has test-only hooks for this (they change nothing in normal use): a fixed time, random seed and battery level, a text mask (each string's box beside a snapshot, `NATIVEMONS_SNAPSHOT_TEXTMASK=1`), `--snapshot-anim` (every frame of an animation), `--replay` (a script of key, pad and mouse input, run with no window, saving pictures as it goes) and a dry run (`NATIVEMONS_TEST_DRYRUN=1`: what a Play would start is written down, never started). The Rust launcher has the same, so both can run the same thing and be compared.

`golden-diff <golden.png> <ours.png>` sorts pixels into classes:
- **Text:** ignored, from both text masks; the text boxes themselves must match within 2 px.
- **Edges** (anti-aliased pixels): may differ more, within limits.
- **Interior:** at most 2 per channel.
- **Shapes:** the outlines of everything drawn must overlap by at least 99.5%.
- **Named residuals:** a few known GDI+ roundings the clone doesn't reproduce exactly, such as a gradient's wrap row and stacked translucent layers, may differ by up to 3. The Rust snapshot names those pixels itself.

The scripts in `launcher-rs/tools`:

| Script | Checks |
| --- | --- |
| `run-goldens.ps1` | Every screen as a snapshot in both launchers, then `golden-diff` per screen. `-CSharp` remakes the C# pictures, `-Anim` compares animations frame by frame. |
| `run-replay.ps1` | The input scripts in `tools/replays` on both launchers: each page and selection step by step, and with `-Frames` every frame drawn. `-DryRun` compares the launch plans of the play flows. |
| `parity-cli.ps1` | The command-line modes on copies of the same inputs, results and written files compared byte for byte. |
| `goldens-*.ps1` | Make the fixtures the others use (ROM copies, Gen1Recomp with sample mods, Pokémon boxes, 3DS builds). |
| `window-test.ps1` | A short live check of the window: frame rate with Light mode off and on, posted keys, scripted mouse, power-off, the tablet. |

The stable check (`tests/stable-check`, [stable-check.md](stable-check.md)) also runs on the Rust exe with `-LauncherOverride`, for the groups that open no window.

The fixtures use copies of your own ROMs and saves, only read. Some of the scripts' default folders are still fixed to one machine's layout: pass your own with their parameters, or change them, before running.

## What's left

Most of the launcher is ported: every screen, the page flow, the command-line modes, the save code, and the play flows for each engine, checked against the C#. Still to come, among others: quick start and title-screen snapshots, title-screen and town captures, the guide overlay, file pickers, and the whole Linux side (building it, native engine builds, a Linux package).

## How it will ship

When it does everything the C# launcher does, it replaces it in place through the normal update: it takes the C# exe's name in the package, so the updater installs it like any other update, with the same folders, state, saves and settings. Players have nothing to download or set up.
