# Engines and what nativeMons is built from

[Back to the README](../README.md)

The two decompilation ports in more detail, and every project nativeMons is built from.

## nativeplat

[nativeplat](https://github.com/sohamb117/gen4recomp) by sohamb117 runs Diamond, Pearl and Platinum natively, ported
from the pret decompilations. It's the only engine for Diamond and Pearl, and an option for Platinum under **Engine**
on the game's page.
- Packages carry a Windows build of the program only, made by [our fork's public workflow](https://github.com/Hailey-Ross/gen4recomp).
  Your ROM stays on your PC.
- Our fork adds quick start.
- It shares one save with the other engines, checked every time it comes back.
- It's experimental, so keep a backup. nativeMons isn't affiliated with nativeplat, so please report problems here
  first.

## Pokémon Platinum PC port

The [Platinum PC port](https://github.com/cybervisi0n/pokeplatinum) by cybervisi0n and contributors is another native
Platinum, from the pret decompilation and [libntr](https://github.com/cybervisi0n/libntr). Choose it with **Engine**
on Platinum's page.
- The download has only the program: its first start unpacks the game's files from your own ROM, on your PC.
- It shares one save with Fulldex, checked every time it comes back.
- It's experimental, so keep a backup. nativeMons isn't affiliated with the port, so please report problems here
  first.

## What it's built from

| Part | What it adds | Source |
|---|---|---|
| `engines/gen1recomp` | Red to Crystal, Ruby, Sapphire, FireRed, LeafGreen, Emerald, and mods. Its own launcher is removed, as its license requires of forks; our fork adds quick start and keeps the launcher's background jobs out of sight | [bryanthaboi/gen1recomp](https://github.com/bryanthaboi/gen1recomp) |
| `engines/fulldex` | DS and GBA games in widescreen at 60 fps. Packages use its published Windows build | [mamesj/pokemon-fulldex](https://github.com/mamesj/pokemon-fulldex), a fork of [ClickingMaster/ssrecomp](https://github.com/ClickingMaster/ssrecomp), using melonDS and mGBA |
| GBA player (`engines/fulldex/gba-wide`) | GBA games and hacks: FireRed, LeafGreen and Emerald in widescreen by our patches; Ruby, Sapphire, other languages and the romhacks PokeDaisy's EXPAND knows (Unbound, Radical Red, Rocket Edition, ..., plus BugLime and Adventure Red, which we added) widened by EXPAND; everything else at 3:2. Ours, in our Fulldex fork; packages build it | [Hailey-Ross/pokemon-fulldex](https://github.com/Hailey-Ross/pokemon-fulldex/tree/hails-additions/gba-wide), after Pokemon GBA Widescreen by mamesj |
| Platinum PC port | A native Platinum. Packages carry its published Windows release, program only | [cybervisi0n/pokeplatinum](https://github.com/cybervisi0n/pokeplatinum) |
| nativeplat [![nativeplat build](https://img.shields.io/github/actions/workflow/status/Hailey-Ross/gen4recomp/windows-release.yml?branch=hails-additions&label=build&style=flat-square&labelColor=6A3FC8)](https://github.com/Hailey-Ross/gen4recomp/releases) | Native Diamond, Pearl and Platinum. Packages carry a build from our fork, program only | [sohamb117/gen4recomp](https://github.com/sohamb117/gen4recomp) |
| 3DSrecomp+Zakuro [![3DSrecomp+Zakuro build](https://img.shields.io/github/actions/workflow/status/Hailey-Ross/zakuro/nativemons-windows-release.yml?branch=hails-additions&label=build&style=flat-square&labelColor=6A3FC8)](https://github.com/Hailey-Ross/zakuro/releases) | Native X, Y, Omega Ruby, Alpha Sapphire, Sun, Moon, Ultra Sun and Ultra Moon. Packages carry Zakuro and our build tool from our fork; each game's code is built on the player's PC | [fearkov/zakuro](https://github.com/fearkov/zakuro) ([ours](https://github.com/Hailey-Ross/zakuro)) and [fearkov/3dsrecomp](https://github.com/fearkov/3dsrecomp) |
| 3DS emulators<br>AzaharPlus [![AzaharPlus build](https://img.shields.io/github/actions/workflow/status/Hailey-Ross/AzaharPlus/nativemons-windows-release.yml?branch=hails-additions&label=build&style=flat-square&labelColor=6A3FC8)](https://github.com/Hailey-Ross/AzaharPlus/releases)<br>Azahar [![Azahar build](https://img.shields.io/github/actions/workflow/status/Hailey-Ross/azahar/nativemons-windows-release.yml?branch=hails-additions&label=build&style=flat-square&labelColor=6A3FC8)](https://github.com/Hailey-Ross/azahar/releases) | X to Ultra Moon. Optional: installed from the launcher, from the Windows releases of our forks (not in the package) | [AzaharPlus/AzaharPlus](https://github.com/AzaharPlus/AzaharPlus) ([ours](https://github.com/Hailey-Ross/AzaharPlus)) and [azahar-emu/azahar](https://github.com/azahar-emu/azahar) ([ours](https://github.com/Hailey-Ross/azahar)) |
| `tools/nds-patcher` | Decrypts `.nds` secure areas | [hails.nds-Patcher](https://github.com/Hailey-Ross/hails.nds-Patcher) |
| `tools/mon-bridge` | Reads saves and moves Pokémon | [PKHeX.Core](https://github.com/kwsch/PKHeX) by Kaphotics and contributors |
| `tools/openhome-bridge` | Converts Pokémon between games that have no real transfer | [OpenHome](https://github.com/andrewbenington/OpenHome) by Andrew Benington, run on Node.js |
| `launcher` | The menu | Window and input code from the steam launcher of [the Skate 3 Rust Engine's hails-additions branch](https://github.com/Hailey-Ross/rusty-trucks/tree/hails-additions) (SK8-ENGINE) |

Pokémon Infinite Fusion isn't part of a package: you import it, or add your own installation.
