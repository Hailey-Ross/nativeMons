# Building from source

[Back to the README](../README.md)

Building nativeMons yourself, and the pre-release check.

Open `launcher\build.bat` to build everything into `build\nativeMons`, ready to run. It needs Git and the [.NET 10 SDK](https://dotnet.microsoft.com/download/dotnet/10.0), and downloads LÖVE, Fulldex's published Windows build, the Platinum PC port's and nativeplat's. With Node, Corepack and Rust too, it builds the OpenHome converter; without them, cross-generation moves use a simpler copy. To build somewhere else, put that folder's path in a `build-output.txt` at the repo root. Rebuilding keeps your settings, saves and `games\` folder.

`launcher\build.bat <output folder>` builds only the launcher and needs only Windows, but it can't run games without the engines. Beside the exe, `state\nativemons.cfg` can set the paths `love`, `gen1recomp`, `fulldex`, `pokeplatinum`, `nativeplat`, `infinitefusion`, `openhome`, `node`, `patcher`, `python`, `pythondir` and `emulators` (where 3DS emulators are installed). Version colors and finishes live in `launcher\games.json`.

## The pre-release check

Before a stable release, `tests/stable-check/stable-check.ps1` tests a release package on copies of your ROMs and saves. It runs in groups, so a failed part can be retested with `-Retest`; its header explains the options.
