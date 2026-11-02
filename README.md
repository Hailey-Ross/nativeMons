# nativeMons

[![Bleeding Edge](https://github.com/Hailey-Ross/nativeMons/actions/workflows/bleeding-edge.yml/badge.svg)](https://github.com/Hailey-Ross/nativeMons/actions/workflows/bleeding-edge.yml)
[![Release](https://github.com/Hailey-Ross/nativeMons/actions/workflows/release.yml/badge.svg)](https://github.com/Hailey-Ross/nativeMons/actions/workflows/release.yml)

One launcher for native Pokémon ports on Windows, with a menu for controller, keyboard and mouse. It starts each game
through its own engine, and the engines stay forks that keep pulling their upstream updates.

Parts of Gen1Recomp inspired this project and helped with research along the way. For the Gen1Recomp engine included here: **Based on the Pokemon Gen 1 Recompilation Project by BOIS CLUB GAMES, LLC (https://github.com/bryanthaboi/gen1recomp).**

No games are included: you need your own ROMs. Not affiliated with or endorsed by Nintendo, Creatures Inc.,
GAME FREAK inc. or The Pokémon Company.

## Supported games

| Generation | Game | Gen1Recomp | Fulldex | Platinum PC port | nativeplat |
|---|---|:-:|:-:|:-:|:-:|
| 1 | Red, Blue, Yellow | ✓ | | | |
| 2 | Gold, Silver, Crystal | ✓ | | | |
| 3 | Ruby, Sapphire | ✓ | | | |
| 3 | FireRed, LeafGreen, Emerald | ✓ | ✓ | | |
| 4 | Diamond, Pearl | | | | ✓ |
| 4 | HeartGold, SoulSilver | | ✓ | | |
| 4 | Platinum | | ✓ | ✓ | ✓ |
| 5 | Black, White, Black 2, White 2 | | ✓ | | |

**Not supported yet:** Generation 6 and later (3DS and Switch games), and ROM hacks.

Where a game has more than one engine it starts in Fulldex, and you can switch on its page. Each engine accepts certain versions of a ROM, mostly the US release: the launcher tells you if yours isn't one. Windows only for now.

## Accepted ROMs

Gen1Recomp and nativeplat only run exact dumps, so they check each ROM's SHA-1. If yours is refused, the game's page shows the hash it found.

| Game | Engine | Accepted SHA-1 |
|---|---|---|
| Red | Gen1Recomp | `ea9bcae617fdf159b045185467ae58b2e4a48b9a` |
| Blue | Gen1Recomp | `d7037c83e1ae5b39bde3c30787637ba1d4c48ce2` |
| Yellow | Gen1Recomp | `cc7d03262ebfaf2f06772c1a480c7d9d5f4a38e1` |
| Gold | Gen1Recomp | `d8b8a3600a465308c9953dfa04f0081c05bdcb94` |
| Silver | Gen1Recomp | `49b163f7e57702bc939d642a18f591de55d92dae` |
| Crystal | Gen1Recomp | `f4cd194bdee0d04ca4eac29e09b8e4e9d818c133`<br>`f2f52230b536214ef7c9924f483392993e226cfb` |
| FireRed | Gen1Recomp | `41cb23d8dccc8ebd7c649cd8fbb58eeace6e2fdc`<br>`dd5945db9b930750cb39d00c84da8571feebf417` |
| LeafGreen | Gen1Recomp | `574fa542ffebb14be69902d1d36f1ec0a4afd71e`<br>`7862c67bdecbe21d1d69ce082ce34327e1c6ed5e` |
| Ruby | Gen1Recomp | `f28b6ffc97847e94a6c21a63cacf633ee5c8df1e`<br>`610b96a9c9a7d03d2bafb655e7560ccff1a6d894`<br>`5b64eacf892920518db4ec664e62a086dd5f5bc8` |
| Sapphire | Gen1Recomp | `3ccbbd45f8553c36463f13b938e833f652b793e4`<br>`4722efb8cd45772ca32555b98fd3b9719f8e60a9`<br>`89b45fb172e6b55d51fc0e61989775187f6fe63c` |
| Emerald | Gen1Recomp | `f3ae088181bf583e55daf962a92bb46f4f1d07b7` |
| Diamond | nativeplat | `a46233d8b79a69ea87aa295a0efad5237d02841e` |
| Pearl | nativeplat | `99083bf15ec7c6b81b4ba241ee10abd9e80999ac` |
| Platinum | nativeplat | `0862ec35b24de5c7e2dcb88c9eea0873110d755c` (Rev 1)<br>`ce81046eda7d232513069519cb2085349896dec7` (Rev 0) |

Fulldex and the Platinum PC port don't check a hash: they go by the game code in the ROM's header, so most dumps of a supported game work.

Decrypt an encrypted DS ROM first: the launcher offers this when it finds one. nativeplat's own import screen refuses Platinum Rev 0, but nativeMons places the ROM itself. The two revisions share the same game files, differing only in a GTS screen, and Platinum runs nativeplat's Rev 1 code either way.

## Download

- **[Bleeding edge](https://github.com/Hailey-Ross/nativeMons/releases/tag/bleeding-edge):** built from every commit to `dev`. It may be broken, so back up your saves.
- **[Stable](https://github.com/Hailey-Ross/nativeMons/releases/tag/stable):** the latest working build, from `main`.

Unzip it anywhere, start `NativeMonsLauncher.exe` and add your ROM folder. Patching `.nds` ROMs needs Python 3.

## Saves

**Several saves per game.** **Saves** on a game's page lets you keep as many saves as you like, each with a name of your choice. Switch between them, start a new one, copy or delete them. A switch keeps your current save exactly as it is, and the engines follow it.

**Bringing your saves.** **Settings > Import saves** finds `.sav` files in your ROM folders and any others you add, and works out their game by reading them. The originals aren't changed, and a save that gets replaced is backed up first.

**One save across engines.** FireRed, LeafGreen and Emerald can run on Gen1Recomp or Fulldex, and Platinum on Fulldex or the PC port. Each of these shares one save when your ROM works with both engines. Before and after each game the launcher brings them level:
- A verified backup comes first. Every save that's about to change is copied, and the copy is checked byte for byte.
- The result has to prove itself: its trainer, party, boxes, Pokédex and bag must match the source, or it's thrown away.
- If that fails, both saves stay as they were, the game still starts, and it tries again next time.
- If both saves changed, you choose which to keep. The other goes to your backups.

A game's page lists its **Save backups** to go back to. After each game the save is checked for corruption, and you're offered a healthy backup if it fails.

**Sending saves back.** **Settings > Export saves**, or a game's page, gives you:
- **A plain `.sav`** for emulators, flash carts and PKHeX.
- **3DS Virtual Console** (Game Boy and Game Boy Color): a Checkpoint folder holding `sav.dat`, copied byte for byte. Matching it to a Checkpoint backup of the game checks the size and copies its `SecureValue`.
- **3DS DS games:** needs a Checkpoint backup of the game, for the exact file name and size.

Only some of this has been tried on a real 3DS, so back up your 3DS save first.

## Pokémon

**My Collection** is a Pokédex built from all the saves of every game the launcher finds. It has a National list from 001 to 649, a list for each region from Kanto to Unova (by the generation that introduced each Pokémon), and your counts by game. Pokémon you haven't seen show as `???`.

A caught Pokémon's entry shows its picture, category, types, height, weight, base stats, abilities and the description from a game you caught it in, plus each game that has it. A gold star marks a shiny, and A switches between its shiny and normal pictures. Shinies are counted only while they're in a party or a box, because saves keep no record of the rest.

Entries come from [PokéAPI](https://pokeapi.co) the first time you open them and are kept in `state\pokedex-cache`. **Settings > Download Pokedex** fetches every picture and entry at once for offline use. No game text is included in nativeMons.

**Move Pokemon** sends a Pokémon from one game's PC boxes to another's, using [PKHeX.Core](https://github.com/kwsch/PKHeX). Both saves are backed up to `state\save-backups` first. If a move is blocked, you choose whether to delete or change the attacks in the way, then retry. A Pokémon that doesn't exist in the destination game can't go there.

Moves with no real transfer between the games use the conversion code from [OpenHome](https://github.com/andrewbenington/OpenHome) (GPL-3.0), and PKHeX checks the result. The result may be flagged as not legal, and its form, ball and origin may change. The original data is kept and comes back when the Pokémon returns to its own generation. This part is experimental: try a moved Pokémon in battle before relying on it.

**Open in PKHeX** and **Open in OpenHome** on a game's page open its save in your own copy of either.

Pokémon pictures come from [pogo.hails.app](https://pogo.hails.app) ([hailsDotGO](https://github.com/Hailey-Ross/hailsDotGO)) and are kept in `state\sprite-cache`, refreshed after 30 days. The launcher fetches a Pokémon along with its whole evolution line, and works without them. **Settings > Download all sprites** downloads every picture for Generations 1 to 5, normal and shiny, for offline play.

## Settings

Each engine has its own page of options, stored where its menus keep them, so changes show up on both sides.

**Updates** checks this project's releases when the launcher starts, and shows on that row when a new build is ready. Updating replaces the launcher and every engine together from one checked download, and changes only the files that differ. Your saves, ROM copies, wallpapers, Pokédex data and settings stay as they are. A copy built from source reads as a local build, and any release counts as newer until you build again.

**Game options** in Settings set volume, music, sound effects, fullscreen, widescreen, smooth scaling, text speed, battle scene, battle style and sound for every game, and a game's own page can override them. Each one is left alone until you choose it, and it applies when a game starts. Not every engine has every setting: the page says which. Options kept in the save aren't available on Generation 5 yet.

## nativeplat

Diamond, Pearl and Platinum can run on [nativeplat](https://github.com/sohamb117/gen4recomp) by sohamb117, native ports from the pret decompilations. It's the only engine for Diamond and Pearl, and an option for Platinum under **Engine** on the game's page. Packages carry a Windows build made by [our fork's public workflow](https://github.com/Hailey-Ross/gen4recomp), program only: your ROM stays on your PC. It shares one save with the other engines, checked each time it comes back. This is experimental, so keep a backup. nativeMons isn't affiliated with that project, so please report problems here first.

## Pokémon Platinum PC port

Platinum can also run on the native [PC port](https://github.com/cybervisi0n/pokeplatinum) by cybervisi0n and contributors, from the pret decompilation and [libntr](https://github.com/cybervisi0n/libntr). Choose it with **Engine** on the game's page. The download has only the program: the first start unpacks the game's files from your own ROM, on your PC. It shares one save with Fulldex, checked each time it comes back. This is experimental, so keep a backup. nativeMons isn't affiliated with that project, so please report problems here first.

## What it's built from

| Part | What it adds | Source |
|---|---|---|
| `engines/gen1recomp` | Red to Crystal, Ruby, Sapphire, FireRed, LeafGreen, Emerald, and mods. Its own launcher is removed, as its licence requires of forks | [bryanthaboi/gen1recomp](https://github.com/bryanthaboi/gen1recomp) |
| `engines/fulldex` | DS and GBA games in widescreen at 60 fps. Packages use its published Windows build | [mamesj/pokemon-fulldex](https://github.com/mamesj/pokemon-fulldex), a fork of [ClickingMaster/ssrecomp](https://github.com/ClickingMaster/ssrecomp), using melonDS and mGBA |
| Platinum PC port | A native Platinum. Packages carry its published Windows release, program only | [cybervisi0n/pokeplatinum](https://github.com/cybervisi0n/pokeplatinum) |
| nativeplat | Native Diamond, Pearl and Platinum. Packages carry a build from our fork, program only | [sohamb117/gen4recomp](https://github.com/sohamb117/gen4recomp) |
| `tools/nds-patcher` | Decrypts `.nds` secure areas | [hails.nds-Patcher](https://github.com/Hailey-Ross/hails.nds-Patcher) |
| `tools/mon-bridge` | Reads saves and moves Pokémon | [PKHeX.Core](https://github.com/kwsch/PKHeX) by Kaphotics and contributors |
| `tools/openhome-bridge` | Converts Pokémon between games that have no real transfer | [OpenHome](https://github.com/andrewbenington/OpenHome) by Andrew Benington, run on Node.js |
| `launcher` | The menu | Window and input code from the steam launcher of [the Skate 3 Rust Engine's hails-additions branch](https://github.com/Hailey-Ross/rusty-trucks/tree/hails-additions) (SK8-ENGINE) |

## Build

Open `launcher\build.bat` to build everything into `build\nativeMons`, ready to run. It needs Git and the [.NET 10 SDK](https://dotnet.microsoft.com/download/dotnet/10.0), and downloads LÖVE, Fulldex's published Windows build, the Platinum PC port's and nativeplat's. With Node, Corepack and Rust too, it builds the OpenHome converter; without them, cross-generation moves use a simpler copy. To build somewhere else, put that folder's path in a `build-output.txt` at the repo root. Rebuilding keeps your settings and saves.

`launcher\build.bat <output folder>` builds only the launcher and needs only Windows, but it can't run games without the engines. Beside the exe, `state\nativemons.cfg` can set the paths `love`, `gen1recomp`, `fulldex`, `pokeplatinum`, `nativeplat`, `openhome`, `node`, `patcher` and `python`. Version colours and finishes live in `launcher\games.json`.

## Credits

Parts of Gen1Recomp inspired this project and helped with research along the way. For the Gen1Recomp engine included here: Based on the Pokemon Gen 1 Recompilation Project by BOIS CLUB GAMES, LLC (https://github.com/bryanthaboi/gen1recomp).

- [Pokémon Fulldex](https://github.com/mamesj/pokemon-fulldex) by mamesj, a fork of [ssrecomp](https://github.com/ClickingMaster/ssrecomp) by ClickingMaster, using [melonDS](https://github.com/melonDS-emu/melonDS) and [mGBA](https://github.com/mgba-emu/mgba).
- [nativeplat](https://github.com/sohamb117/gen4recomp) by sohamb117, from the pret decompilations. Not affiliated with nativeMons.
- [Pokémon Platinum PC port](https://github.com/cybervisi0n/pokeplatinum) by cybervisi0n and contributors, using [libntr](https://github.com/cybervisi0n/libntr) (MIT), from the [pret](https://github.com/pret/pokeplatinum) decompilation. They aren't affiliated with nativeMons.
- [PKHeX.Core](https://github.com/kwsch/PKHeX) by Kaphotics and contributors.
- [OpenHome](https://github.com/andrewbenington/OpenHome) by Andrew Benington (GPL-3.0), running on [Node.js](https://nodejs.org).
- [LÖVE](https://github.com/love2d/love), which runs Gen1Recomp.
- [hails.nds-Patcher](https://github.com/Hailey-Ross/hails.nds-Patcher) by Hails.
- Pokédex entries from [PokéAPI](https://pokeapi.co).
- Pokémon pictures served by [hailsDotGO](https://github.com/Hailey-Ross/hailsDotGO) at pogo.hails.app, from the [PokeAPI sprites](https://github.com/PokeAPI/sprites) collection.
- Launcher design from the Skate 3 Rust Engine's [hails-additions](https://github.com/Hailey-Ross/rusty-trucks/tree/hails-additions) branch.

Not affiliated with or endorsed by Nintendo, Creatures Inc., GAME FREAK inc. or The Pokémon Company. Pokémon and all related names belong to their owners.

## License

GPL-3.0-or-later (see `LICENSE`). Each engine keeps its own licence and credits, including the additional terms in
Gen1Recomp's `LICENSE.MD`.
