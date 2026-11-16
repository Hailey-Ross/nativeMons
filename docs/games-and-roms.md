# Games and ROMs

[Back to the README](../README.md)

Which dumps each engine takes, what happens when you have several versions of one game, and ROM hacks.

## Which engine a game starts with

A game with more than one engine starts in Fulldex; switch engines on its page. Each engine accepts certain versions
of a ROM, mostly the US release, and the launcher tells you if yours isn't one. nativeMons runs on Windows only for
now.

## Accepted ROMs

**Gen1Recomp and nativeplat** only run exact dumps, so they check each ROM's SHA-1. If yours is refused, the game's
page shows the hash it found.

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

**Fulldex and the Platinum PC port** don't check a hash. They go by the game code in the ROM's header, so most dumps
of a supported game work.

**3DS games** (`.3ds` or `.cci` cartridge dumps) are found by the title ID in their header, so any dump of a
supported game works. Azahar needs a decrypted dump (GodMode9 on your 3DS makes one). An encrypted dump always plays
with AzaharPlus, even when Azahar is the emulator you picked.

**An encrypted DS ROM** needs decrypting first, and the launcher offers that when it finds one.

**Platinum Rev 0:** nativeplat's own import screen refuses it, but nativeMons places the ROM itself. The two
revisions share the same game files, differing only in a GTS screen, and nativeplat runs Rev 1's code either way.

## Several versions of a game

If your folders hold more than one version of a game, such as Rev 0 and Rev 1, the game's page gets a **ROM
version** row to choose between them.

## ROM hacks

A ROM hack is a game of its own. A hack of a game Fulldex runs (FireRed, LeafGreen and Emerald; Platinum hacks should
work the same way, but haven't been tried) gets its own place in **Games**:
- **Its name** comes from its file: `Pokemon_FireRed_Rocket_Edition.gba` shows as "Pokemon FireRed Rocket Edition".
  **Rename** on its page changes it, and the new name stays even if the file is renamed.
- **What counts as a hack** is decided by the ROM's contents alone: it carries the game's code but isn't one of that
  game's clean dumps (every clean revision is known). So a clean copy stays with its game whatever its file is
  called.
- **It runs on Fulldex only**, with saves, backups and save slots of its own. Its saves read like the base game's, so
  **Move Pokemon**, **My Collection** and PKHeX work with them.
- **Widescreen** depends on the hack.

Tested: [Pokémon Rocket Edition](https://rocket-edition.com/) FINAL v1.0.2, a FireRed hack. It opens straight into
the game and plays with no graphical issues.
