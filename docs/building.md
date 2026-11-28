# Building from source

[Back to the README](../README.md)

Building nativeMons yourself, the Rust launcher that isn't released yet, and the check run before every stable
release.

## Everything

Open `launcher\build.bat` to build everything into `build\nativeMons`, ready to run.
- **It needs** Git and the [.NET 10 SDK](https://dotnet.microsoft.com/download/dotnet/10.0).
- **It downloads** LÖVE, and the published Windows builds of Fulldex, the Platinum PC port, nativeplat and
  3DSrecomp+Zakuro.
- **With Node, Corepack and Rust** installed too, it builds the OpenHome converter. Without them, moves between
  generations use a simpler copy.
- **To build somewhere else,** put that folder's path in a `build-output.txt` at the repo root.
- **Rebuilding** keeps your settings, saves and `games\` folder.

## Only the launcher

`launcher\build.bat <output folder>` builds just the launcher, and needs nothing but Windows. It can't run games
without the engines, so point it at them: beside the exe, `state\nativemons.cfg` can set these paths, one
`name=path` a line:

`love`, `gen1recomp`, `fulldex`, `gbawide`, `mgbacore`, `pokeplatinum`, `nativeplat`, `zakuro`, `infinitefusion`,
`infinitefusion2`, `games`, `openhome`, `node`, `monbridge`, `patcher`, `python`, `pythondir` and `emulators` (where
3DS emulators are installed).

Each game version's colors and finishes live in `launcher\games.json`.

## The Rust launcher (not released yet)

`launcher-rs/` is a Cargo workspace with the Rust rebuild of the launcher. Packages don't include it yet, and
`launcher\build.bat` doesn't build it. [notes/rust-launcher.md](../notes/rust-launcher.md) explains its parts and
how it's checked against the C# launcher.
- **It needs** [Rust](https://rustup.rs) (stable) and a C compiler: on Windows, the MSVC Build Tools (with the
  `x86_64-pc-windows-msvc` toolchain). The windowed build also needs [CMake](https://cmake.org), because it builds
  SDL3 from source and links it in. If CMake isn't on your `PATH`, set `CMAKE` to its `cmake.exe` (Visual Studio
  includes one), or build from a Developer prompt.
- **Build output:** set `CARGO_TARGET_DIR` to a folder outside the repo, so the build doesn't land in your checkout.
- **Builds:**
  - `cargo build --release -p nativemons` builds it without a window: the command-line modes and `--snapshot`.
  - `cargo build --release -p nativemons --features window` builds the launcher with its window.
  - `cargo build -p golden-diff` builds the picture comparer the parity scripts use.
- **To run it,** put `nativemons.exe` in a folder with `launcher\games.json` beside it and a `fonts` folder holding
  `launcher\fonts` and `launcher-rs\fonts` (or point `NATIVEMONS_FONTS_DIR` at such a folder). Like the C# exe, it
  needs a built nativeMons folder's engines to run games. Don't use it on a folder with saves you care about yet.
- **Tests:** `cargo test -p nm-core -p nm-ui -p nm-platform --lib`, with `NATIVEMONS_FONTS_DIR` set for the drawing
  tests. The parity scripts are in `launcher-rs\tools` (see the notes).

So far it has been built and tested on Windows only; its Linux side hasn't been built yet.

## The pre-release check

Before a stable release, `tests/stable-check/stable-check.ps1` tests a release package on copies of your ROMs and
saves. It runs in groups, so a failed part can be retested on its own with `-Retest`. Its header explains the
options.
