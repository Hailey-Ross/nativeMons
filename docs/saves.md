# Saves

[Back to the README](../README.md)

Save slots, importing and exporting saves, one save across engines, and backups.

**Several saves per game.** **Saves** on a game's page lets you keep as many saves as you like, each with a name of your choice. Switch between them, start a new one, copy or delete them. A switch keeps your current save exactly as it is, and the engines follow it.

**Bringing your saves.** **Settings > Import saves** finds `.sav` files in your ROM folders and any others you add, and works out their game by reading them. The originals aren't changed, and a save that gets replaced is backed up first.

**One save across engines.** FireRed, LeafGreen and Emerald can run on Gen1Recomp or Fulldex, and Platinum on Fulldex, nativeplat or the PC port. Each of these shares one save when your ROM works with the engines. Before and after each game the launcher brings them level:
- A verified backup comes first. Every save that's about to change is copied, and the copy is checked byte for byte.
- The result has to prove itself: its trainer, party, boxes, Pokédex and bag must match the source, or it's thrown away.
- If that fails, both saves stay as they were, the game still starts, and it tries again next time.
- If both saves changed, you choose which to keep. The other goes to your backups.

**Everything stays in the launcher's folder.** Saves, settings, mods and prepared games live beside `NativeMonsLauncher.exe`, in `state` and in each engine's folder, so the whole folder can move to another drive or PC. Gen1Recomp kept its own in `%APPDATA%\LOVE\nativemons` before; the first start of a newer launcher copies them into `state\gen1recomp`, checks every file, and leaves the old folder as it was, as a backup you can delete. If that copy can't finish, Gen1Recomp keeps using the old folder and the launcher tries again next time. Pokémon Infinite Fusion, when you add your existing installation, keeps its saves where the game puts them.

A game's page lists its **Save backups** to go back to. After each game the save is checked for corruption, and you're offered a healthy backup if it fails.

**Sending saves back.** **Settings > Export saves**, or a game's page, gives you:
- **A plain `.sav`** for emulators, flash carts and PKHeX.
- **3DS Virtual Console** (Game Boy and Game Boy Color): a Checkpoint folder holding `sav.dat`, copied byte for byte. Matching it to a Checkpoint backup of the game checks the size and copies its `SecureValue`.
- **3DS DS games:** needs a Checkpoint backup of the game, for the exact file name and size.

Only some of this has been tried on a real 3DS, so back up your 3DS save first.
