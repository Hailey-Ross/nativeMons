# nativeMons

[![Bleeding Edge](https://github.com/Hailey-Ross/nativeMons/actions/workflows/bleeding-edge.yml/badge.svg)](https://github.com/Hailey-Ross/nativeMons/actions/workflows/bleeding-edge.yml)
[![Release](https://github.com/Hailey-Ross/nativeMons/actions/workflows/release.yml/badge.svg)](https://github.com/Hailey-Ross/nativeMons/actions/workflows/release.yml)

One launcher for native Pokémon ports on Windows, with a menu for controller, keyboard and mouse. It starts each game
through its own engine, and the engines stay forks that keep pulling their upstream updates.

**Based on the Pokemon Gen 1 Recompilation Project by BOIS CLUB GAMES, LLC (https://github.com/bryanthaboi/gen1recomp).**

No games are included: you need your own ROMs. Not affiliated with or endorsed by Nintendo, Creatures Inc.,
GAME FREAK inc. or The Pokémon Company.

| Part | What it adds | Source |
|---|---|---|
| `engines/gen1recomp` | Red to Crystal, Ruby, Sapphire, FireRed, LeafGreen, Emerald, and mods. Its own launcher is removed, as its licence requires of forks | [bryanthaboi/gen1recomp](https://github.com/bryanthaboi/gen1recomp) |
| Pokémon Fulldex | DS and GBA games in widescreen at 60 fps | [mamesj/pokemon-fulldex](https://github.com/mamesj/pokemon-fulldex), a fork of [ClickingMaster/ssrecomp](https://github.com/ClickingMaster/ssrecomp), using melonDS and mGBA |
| `tools/nds-patcher` | Decrypts `.nds` secure areas | [hails.nds-Patcher](https://github.com/Hailey-Ross/hails.nds-Patcher) |
| `tools/mon-bridge` | Reads saves and moves Pokémon | [PKHeX.Core](https://github.com/kwsch/PKHeX) by Kaphotics and contributors |
| `launcher` | The menu | Window and input code from the steam launcher of [the Skate 3 Rust Engine's hails-additions branch](https://github.com/Hailey-Ross/rusty-trucks/tree/hails-additions) (SK8-ENGINE) |

## Download
- **[Bleeding edge](https://github.com/Hailey-Ross/nativeMons/releases/tag/bleeding-edge):** built from every commit to `dev`. It may be broken, so back up your saves.
- **[Stable](https://github.com/Hailey-Ross/nativeMons/releases/tag/stable):** the latest working build, from `main`.

Unzip it anywhere, start `NativeMonsLauncher.exe` and add your ROM folder. Patching `.nds` ROMs needs Python 3.

## My Collection
Tallies every installed game: the different Pokémon you've caught across all your saves, against the National Dex entries your games cover. A new game adds any entries you didn't have yet.

## Bringing your saves
**Settings > Import saves** finds `.sav` files in your ROM folders and any others you add, and works out their game by reading them. The originals aren't changed, and a save that gets replaced is backed up first.

## One save for both engines
FireRed, LeafGreen and Emerald can run on either engine, and share one save when your ROM works with both. Before and after each game the launcher brings them level:
- A verified backup comes first. Every save that's about to change is copied, and the copy is checked byte for byte.
- The result has to prove itself: its trainer, party, boxes, Pokédex and bag must match the source, or it's thrown away.
- If that fails, both saves stay as they were, the game still starts, and it tries again next time.
- If both saves changed, you choose which to keep. The other goes to your backups.

A game's page lists its **Save backups** to go back to. After each game the save is checked for corruption, and you're offered a healthy backup if it fails.

## Sending saves back
**Settings > Export saves**, or a game's page, gives you:
- **A plain `.sav`** for emulators, flash carts and PKHeX.
- **3DS Virtual Console** (Game Boy and Game Boy Color): a Checkpoint folder holding `sav.dat`, copied byte for byte. Matching it to a Checkpoint backup of the game checks the size and copies its `SecureValue`.
- **3DS DS games:** needs a Checkpoint backup of the game, for the exact file name and size.

Only some of this has been tried on a real 3DS, so back up your 3DS save first.

## Settings
One place for both engines: fullscreen and volume for every game, plus each engine's own options. They're stored where each engine's menus keep them, so changes show up on both sides.

## Several saves per game
**Saves** on a game's page lets you keep as many saves as you like, each with a name of your choice. Switch between them, start a new one, copy or delete them. A switch keeps your current save exactly as it is, and both engines follow it. **My Collection** counts each Pokémon once across all of a game's saves.

## Pokémon Platinum PC port
Platinum can also run on the native [PC port](https://github.com/cybervisi0n/pokeplatinum) by cybervisi0n and contributors, from the pret decompilation and [libntr](https://github.com/cybervisi0n/libntr). Choose it with **Engine** on the game's page. The download has only the program: the first start unpacks the game's files from your own ROM, on your PC. It shares one save with Fulldex, checked each time it comes back. nativeMons isn't affiliated with that project, so please report problems here first.

## Moving Pokémon
**Move Pokemon** sends a Pokémon from one game's PC boxes to another's, using [PKHeX.Core](https://github.com/kwsch/PKHeX). Both saves are
backed up to `state\save-backups` first. If a move is blocked, you choose whether to delete or change the attacks in the way, then retry. A Pokémon that doesn't exist in the destination game can't go there, and moves with no real transfer between those games may come out flagged as not legal. **Open in PKHeX**
on a game's page opens its save in your own copy of PKHeX.

Pokémon pictures come from [pogo.hails.app](https://pogo.hails.app) ([hailsDotGO](https://github.com/Hailey-Ross/hailsDotGO)) and are kept in `state\sprite-cache`, refreshed after 30 days. The launcher fetches a Pokémon along with its whole evolution line, and works without them. **Settings > Download all sprites** downloads every picture for Generations 1 to 5, normal and shiny, for offline play.

## Build and set up
Open `launcher\build.bat` to build everything into `build\nativeMons`, ready to run. It needs Git and the [.NET 10 SDK](https://dotnet.microsoft.com/download/dotnet/10.0), and downloads LÖVE and Fulldex's published Windows build. To build somewhere else, put that folder's path in a `build-output.txt` at the repo root. Rebuilding keeps your settings and saves.

`launcher\build.bat <output folder>` builds only the launcher and needs only Windows, but it can't run games without the engines. Beside the exe, `state\nativemons.cfg` can set the paths `love`,
`gen1recomp`, `fulldex`, `patcher` and `python`. Version colours and finishes live in `launcher\games.json`.

GPL-3.0-or-later (see `LICENSE`). Each engine keeps its own licence and credits, including the additional terms in
Gen1Recomp's `LICENSE.MD`.
