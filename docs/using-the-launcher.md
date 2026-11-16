# Using the launcher

[Back to the README](../README.md)

Getting around the menus, adding games by dropping files on the window, shortcuts, and what happens the first time
you start it.

## Getting around

The launcher looks like a phone-sized Pokédex in the colors of the game you're on, and unfolds into a tablet for
**Move**. The home screen shows the game you played last: its intro plays in the **Play** tile, and one of its towns
is the wallpaper, both captured from your own ROM on your PC. Below are **Games**, **Collection**, **Mods**,
**ROMs**, **Emu** (the 3DS emulators), **Move**, **Settings** and **Exit**.

| Control | Move | Select | Back |
|---|---|---|---|
| Controller | D-pad or left stick | A | B |
| Keyboard | Arrow keys or WASD | Enter | Esc |
| Mouse | Hover | Click | Right-click |

With the mouse, you can also click the device's own D-pad and buttons, and drag its shell to move the window.

A few things to look out for:
- The highlighted app's icon moves (the Games Poké Ball shakes as if mid-catch), and opening an app grows its icon to
  fill the screen.
- A spinning rainbow Poké Ball means the launcher is working on something: starting up, preparing a game, backing up
  a save, making a title screen, or fetching Pokémon pictures and Pokédex entries.
- Closing the launcher powers it down like an old device: the screen shrinks to a dot, the lights go out, and it
  drops off the bottom of the screen.

A game's page in **Games** has **Play**, its engine and ROM version, its saves, game options, mods, backups and
export, and the **PKHeX** and **OpenHome** buttons.

## Adding games by dropping files

Drop files on the window from any menu screen:
- **A folder** becomes one of your ROM folders.
- **A ROM** (`.gb`, `.gbc`, `.gba`, `.nds`, `.3ds`, `.cci`) is used where it is, without the rest of its folder.
- **A `.zip`**: its ROMs are copied into `state\roms`, and the zip itself isn't changed.
- **A `.sav`** goes to **Import saves**.

Afterward, a page says which game each ROM turned out to be. If a ROM isn't one nativeMons runs, the page says so
and shows its SHA-1. **ROMs > Add a ROM file** does the same from a file picker. ROMs added on their own, and copies
taken from a zip, are listed on the **ROMs** page, where you can remove them.

## Shortcuts

`NativeMonsLauncher.exe --game <id>` starts a game straight away, and closes the launcher when the game ends. The id
is the game's name in lowercase, such as `firered`, `crystal` or `black2`.
- `--slot <number or name>` picks which of the game's saves to use.
- `--launcher` opens the menu on that game's page instead of starting it.
- If the launcher is already open, it takes the request itself.
- `--play <id>` still works too.

## The first start

The first start after adding ROMs takes about a minute behind the splash screen:
- Your Game Boy and GBA games are prepared for Gen1Recomp from your ROMs, three at a time.
- The game the menu opens on gets its title screen and town wallpaper. The other games get theirs in the background
  once the menu is up.

All of this runs out of sight at low priority. Most of the load is Windows Defender scanning the many small files
this writes. After that, the launcher opens in about a second.
