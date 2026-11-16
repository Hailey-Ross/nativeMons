# Using the launcher

[Back to the README](../README.md)

Moving around the menus, adding games by dropping files, shortcuts, and what the first start does.

## Getting around

The launcher is a phone-sized Pokédex that takes on each game's colors, and unfolds into a tablet for **Move**. Its home screen shows the game you played last: its intro plays in the **Play** tile, and one of its towns is the wallpaper, both captured from your own ROM on your PC. Below that are **Games**, **Collection**, **Mods**, **ROMs**, **Emu** (the 3DS emulators), **Move**, **Settings** and **Exit**. The highlighted app's icon moves (the Games Poké Ball shakes as if mid-catch), and opening one grows its icon to fill the screen before the new screen's rows pop in. Closing the launcher powers it down like an old device: the screen shrinks to a dot, the lights flicker out, and it drops away off the bottom of the screen. Whenever the launcher is loading, a rainbow Poké Ball spins: on the splash at start, while it works on something such as preparing a game or backing up a save, while a game's title screen is being made in its Play tile, and while Pokémon pictures and Pokédex entries arrive.

- **D-pad or left stick:** move. **A:** select. **B:** back.
- **Keyboard:** arrow keys or WASD, Enter, Esc.
- **Mouse:** hover to select, click to choose, right-click to go back. The D-pad and buttons on the device can be clicked too, and dragging its shell moves the window.

**Dropping files on the window** works on any menu screen. A folder becomes a ROM folder. A `.gb`, `.gbc`, `.gba`, `.nds`, `.3ds` or `.cci` ROM is used where it is, without its folder. A `.zip`'s ROMs are copied into `state\roms`, and the zip isn't changed. A `.sav` goes to Import saves. Afterwards a page says which game each ROM turned out to be. If a ROM isn't one nativeMons runs, the page says so and shows its SHA-1. **ROMs > Add a ROM file** does the same from a file picker. ROMs added on their own, and copies taken from a zip, are listed on the ROMs page, where you can remove them.

**Shortcuts:** `NativeMonsLauncher.exe --game <id>` starts a game and closes the launcher when the game ends. Ids are the lowercase names, such as `firered`, `crystal` or `black2`. Add `--slot <number or name>` to choose which of its saves to use, or `--launcher` to open the menu on that game's page instead. If the launcher is already open, it takes the request itself. `--play <id>` still works.

A game's page in **Games** has Play, its engine, ROM version, saves, game options, mods, backups, export and the PKHeX and OpenHome buttons.

## The first start

The first start after adding ROMs takes about a minute behind the splash screen:
- Your Game Boy and GBA games are prepared for Gen1Recomp from your ROMs, three at a time.
- The game the menu opens on gets its title screen and town wallpaper; the other games' pictures are made in the background once the menu is up.

Everything runs out of sight at low priority. Windows Defender scans the many small files preparing writes, which is most of the load during that first start. After that the launcher opens in about a second.
