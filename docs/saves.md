# Saves

[Back to the README](../README.md)

Save slots, bringing saves in and sending them back out, one save across engines, and backups.

## Save slots

**Saves** on a game's page keeps as many saves as you like, each with a name of your choice. Switch between them,
start a new one, or copy or delete one. Switching keeps your current save exactly as it is, and every engine follows
the switch.

## Bringing your saves in

**Settings > Import saves** finds `.sav` files in your ROM folders and any other folders you add, and works out which
game each one is by reading it. Your originals aren't changed, and any save that gets replaced is backed up first.

## One save across engines

FireRed, LeafGreen and Emerald can run on Gen1Recomp or Fulldex, and Platinum on Fulldex, nativeplat or the PC port.
When your ROM works with those engines, the game shares one save across them. Before and after each session, the
launcher brings the engines' copies level:
- **A verified backup comes first.** Every save about to change is copied, and the copy is checked byte for byte.
- **The result has to prove itself.** Its trainer, party, boxes, Pokédex and bag must match the source, or it's
  thrown away.
- **If that fails,** both saves stay as they were, the game still starts, and the launcher tries again next time.
- **If both saves changed,** you choose which to keep, and the other goes to your backups.

## Backups

A game's page lists its **Save backups** to go back to. After each session, the save is checked for corruption, and
if it fails, you're offered a healthy backup.

## Sending saves back out

**Settings > Export saves**, or a game's page, gives you:
- **A plain `.sav`** for emulators, flash carts and PKHeX.
- **3DS Virtual Console** (Game Boy and Game Boy Color games): a Checkpoint folder holding `sav.dat`, copied byte for
  byte. Matching it to a Checkpoint backup of the game checks the size and copies its `SecureValue`.
- **DS games on a 3DS:** needs a Checkpoint backup of the game, for the exact file name and size.

Only some of this has been tried on a real 3DS, so back up your 3DS save first.

## Where everything lives

Saves, settings, mods and prepared games all live beside `NativeMonsLauncher.exe` (in `state` and in each engine's
folder), so you can move the whole folder to another drive or PC.
- **Gen1Recomp** used to keep its saves in `%APPDATA%\LOVE\nativemons`. The first start of a newer launcher copies
  them into `state\gen1recomp`, checks every file, and leaves the old folder as it was, as a backup you can delete.
  If the copy can't finish, Gen1Recomp keeps using the old folder, and the launcher tries again next time.
- **Pokémon Infinite Fusion**, when you add an existing installation, keeps its saves where the game puts them.
