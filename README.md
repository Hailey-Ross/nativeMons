# nativeMons

[![Bleeding Edge](https://github.com/Hailey-Ross/nativeMons/actions/workflows/bleeding-edge.yml/badge.svg)](https://github.com/Hailey-Ross/nativeMons/actions/workflows/bleeding-edge.yml)

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
- **Stable:** built from `main`, once a working build has been promoted there.

Unzip it anywhere, start `NativeMonsLauncher.exe` and add your ROM folder. Patching `.nds` ROMs needs Python 3.

## My Collection
Tallies every installed game: the different Pokémon you've caught across all your saves, against the National Dex entries your games cover. A new game adds any entries you didn't have yet.

## Settings
One place for both engines: fullscreen and volume for every game, plus each engine's own options. They're stored where each engine's menus keep them, so changes show up on both sides.

## Moving Pokémon
**Move Pokemon** sends a Pokémon from one game's PC boxes to another's, using [PKHeX.Core](https://github.com/kwsch/PKHeX). Both saves are
backed up to `state\save-backups` first. If a move is blocked, you choose whether to delete or change the attacks in the way, then retry. A Pokémon that doesn't exist in the destination game can't go there, and moves with no real transfer between those games may come out flagged as not legal. **Open in PKHeX**
on a game's page opens its save in your own copy of PKHeX.

Pokémon pictures come from [pogo.hails.app](https://pogo.hails.app) ([hailsDotGO](https://github.com/Hailey-Ross/hailsDotGO)) and are kept in `state\sprite-cache`, refreshed after 30 days. The launcher fetches a Pokémon along with its whole evolution line, and works without them. **Settings > Download all sprites** downloads every picture for Generations 1 to 5, normal and shiny, for offline play.

## Build and set up
`launcher\build.bat <output folder>` needs only Windows. Beside the exe, `state\nativemons.cfg` can set the paths `love`,
`gen1recomp`, `fulldex`, `patcher` and `python`. Version colours and finishes live in `launcher\games.json`.

GPL-3.0-or-later (see `LICENSE`). Each engine keeps its own licence and credits, including the additional terms in
Gen1Recomp's `LICENSE.MD`.
