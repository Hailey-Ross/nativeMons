# Linux and Steam Deck (early)

[Back to the README](../README.md)

> **Early and not ready for everyone.** nativeMons is a Windows program. On Linux and the Steam Deck it runs
> under Proton or Wine, and only part of that has been tried, on a desktop stand-in rather than a real Deck.
> Expect rough edges, and keep backups of your saves. If you're not comfortable with a terminal, wait for a later
> release.

## What works so far

- The launcher starts under Proton (through umu-launcher) or Wine, with smooth drawing.
- Menus, adding a ROM folder, finding your games, title screens and wallpapers.
- Gen1Recomp games start from the launcher: Red started fullscreen at 1280x800 under GE-Proton.
- On short landscape screens, such as the Deck's 1280x800, the launcher opens in its **wide layout** on its own:
  the menu in a column on the left and the chosen option shown large beside it.

## Not tried or not working yet

- A real Steam Deck, and Game Mode.
- Fulldex, nativeplat, the Platinum PC port and the GBA player under Wine.
- 3DSrecomp+Zakuro: building a 3DS game under Wine.
- The file and folder pickers, which need a mouse and keyboard.
- Discord status, and "Open folder" buttons.
- The Steam button doesn't reach the launcher, so the guide overlay needs the keyboard or another button.
- Some menu icons may show as boxes.

## Starting it

You need **umu-launcher** (recommended, it runs Proton) or **Wine**, plus `curl` (or `wget`) and `cabextract` (or
7-Zip).

1. Download the Windows zip and unzip it into a folder in your home folder, such as `~/Applications/nativeMons`.
2. In a terminal in that folder, run `chmod +x nativemons.sh`, then `./nativemons.sh`.

The first start sets up a Windows prefix in `~/.local/share/nativeMons/prefix`, which takes a minute.

**Settings, if you need them:**
- `WINEPREFIX`: use another prefix.
- `NATIVEMONS_RUNNER=wine` or `NATIVEMONS_RUNNER=umu`: choose the runner yourself.
- `PROTONPATH`: choose the Proton version that umu-launcher uses.

## Microsoft's GDI+

Wine's own GDI+ draws the launcher with rough edges, and crashes under Proton 10. So on the first start,
`nativemons.sh` fetches Microsoft's GDI+ security update **KB3045171** (4.2 MB) from Microsoft's own update server.

- It checks the download's checksum, unpacks only `gdiplus.dll` without running anything, and checks that file too.
- It puts the file in the prefix and starts the launcher with it.
- The file is never part of the nativeMons download. Each player gets it from Microsoft.
- Each start checks it again, because a Proton update can replace it.

To skip it, start with `NATIVEMONS_NO_GDIPLUS=1`. The launcher still works, with rougher drawing.

## As a non-Steam game

To start it from Steam, add `NativeMonsLauncher.exe` as a non-Steam game, force a Proton version in its
compatibility settings, and start it once so Steam makes its prefix. Then:

1. Find the game's prefix in `~/.steam/steam/steamapps/compatdata/<id>/pfx`.
2. Run `./nativemons.sh --setup-prefix <that pfx folder>`.
3. Set the game's launch options to `WINEDLLOVERRIDES="gdiplus=n,b" %command%`.

## Reporting problems

Say it's Linux or a Steam Deck, with your distribution, and Proton or Wine with its version.
[**New issue**](https://github.com/Hailey-Ross/nativeMons/issues/new)
