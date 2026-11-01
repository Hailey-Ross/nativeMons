## Setup

1. **You need** Windows 10 or 11 (64-bit) and your own ROMs. No games are included. `.nds` ROMs that need patching also need [Python 3](https://www.python.org/downloads/).
2. **Download** the zip above and extract it to a folder of its own (not inside Program Files).
3. **Start** `NativeMonsLauncher.exe`. Choose **ROM folders** and add the folder with your `.gb`, `.gbc`, `.gba` and `.nds` files. File names don't matter: games are found by reading each ROM.
4. **Your saves:** **Settings > Import saves** brings in saves from other emulators. The originals aren't changed.

The first time you play a Game Boy or GBA game with Gen1Recomp, the launcher prepares it from your ROM, which takes a minute or two.

## Games

| Engine | Games |
|---|---|
| **Gen1Recomp** (native, with mods) | Red, Blue, Yellow, Gold, Silver, Crystal, Ruby, Sapphire, FireRed, LeafGreen, Emerald |
| **Fulldex** (widescreen, 60 fps) | FireRed, LeafGreen, Emerald, HeartGold, SoulSilver, Platinum, Black, White, Black 2, White 2 |

Where both engines have a game, it starts in Fulldex. Switch on that game's page. Each engine keeps its own save.

## Controls

- **D-pad or left stick:** move. **A:** select. **B:** back.
- **Keyboard:** arrow keys or WASD, Enter, Esc.
- **Mouse:** hover to select, click to choose, right-click to go back. The D-pad and buttons on the device can be clicked too, and dragging its shell moves the window.

## What's in it

- **One launcher** for every game, shaped like a handheld device, taking on each version's colours. Gold, Silver, Crystal, Red, Blue and Yellow each have their own scene.
- **My Collection:** the different Pokémon you've caught across all your saves, against the Pokédex entries your games cover.
- **Move Pokémon** between games from their PC boxes, using PKHeX.Core, with both saves backed up first. If attacks or a held item stand in the way, you choose what to delete or change, then retry.
- **Import and export saves:** plain `.sav` files for emulators and flash carts, and Checkpoint folders for the 3DS.
- **Open in PKHeX:** a game's page opens its save in your own copy of PKHeX.
- **Settings** for both engines in one place, including **Download all sprites** for offline play.
- **Mods** for Gen1Recomp games, and **hails.nds-Patcher** for `.nds` ROMs that won't load.

## Testing and reports

- One problem per report if you can, with the game, what you did and what you expected. A screenshot helps a lot.
- **Back up your saves** before moving Pokémon or restoring to a device.
- Report here: [**New issue**](https://github.com/Hailey-Ross/nativeMons/issues/new). Don't attach ROMs.

**Please test:** a controller through every screen, moving Pokémon between your own saves, and restoring an exported save on a real 3DS.

## Known issues

- **GBA games in Fulldex:** quitting one shows Fulldex's own game list. Close that to return to the launcher.
- **Hand-copied Pokémon:** moves with no real transfer between the games, such as Gen 5 to Gen 3, copy the Pokémon across by hand. Its form, ball and origin may change, and PKHeX may flag it as not legal. Try one in battle before relying on it.
- **3DS export:** only part of this has been tried on a real 3DS. Game Boy saves from another emulator can't be exported for Virtual Console, because they carry that emulator's clock.
- **Gen1Recomp launcher features** with no replacement yet: carts, box tools, skins and mod asset-pack import. Online play is removed.
- **My Collection** counts National Dex entries, so the total is the largest Pokédex among your games.

## Credits

Based on the Pokemon Gen 1 Recompilation Project by BOIS CLUB GAMES, LLC (https://github.com/bryanthaboi/gen1recomp).

[Pokémon Fulldex](https://github.com/mamesj/pokemon-fulldex) by mamesj, a fork of [ssrecomp](https://github.com/ClickingMaster/ssrecomp), using [melonDS](https://melonds.kuribo64.net) and [mGBA](https://mgba.io). [PKHeX](https://github.com/kwsch/PKHeX) by Kaphotics and contributors. Sprites served by [hailsDotGO](https://pogo.hails.app). Launcher design from the Skate 3 Rust Engine's [hails-additions](https://github.com/Hailey-Ross/rusty-trucks/tree/hails-additions) branch.

Not affiliated with or endorsed by Nintendo, Creatures Inc., GAME FREAK inc. or The Pokémon Company.
