# Notes

How nativeMons works under the hood: the detail that would otherwise crowd the code. The code points here by file and section.

- [launcher.md](launcher.md): the launcher (`launcher/NativeMonsLauncher.cs`): command line, engines, saves, mods, PC games, pictures, updates.
- [mon-bridge.md](mon-bridge.md): `tools/mon-bridge`, which reads saves and moves Pokemon with PKHeX.Core.
- [engine-sync.md](engine-sync.md): the nightly workflow that pulls upstream engine updates and moves the pins.
- [stable-check.md](stable-check.md): `tests/stable-check`, the suite a package passes before it becomes the stable build.
