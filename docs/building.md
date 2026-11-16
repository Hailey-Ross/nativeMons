# Building from source

[Back to the README](../README.md)

Building nativeMons yourself, and the check run before every stable release.

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

## The pre-release check

Before a stable release, `tests/stable-check/stable-check.ps1` tests a release package on copies of your ROMs and
saves. It runs in groups, so a failed part can be retested on its own with `-Retest`. Its header explains the
options.
