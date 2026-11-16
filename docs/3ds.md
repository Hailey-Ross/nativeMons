# 3DS games

[Back to the README](../README.md)

X, Y, Omega Ruby, Alpha Sapphire, Sun, Moon, Ultra Sun and Ultra Moon play best natively, with
[3DSrecomp+Zakuro](#3dsrecompzakuro). It comes with nativeMons and is the recommended way. A 3DS emulator is an
optional fallback, for encrypted dumps or if you'd rather use one.

## For every 3DS game

- **Your ROMs:** `.3ds` or `.cci` dumps of your own cartridges (GodMode9 on your 3DS makes them). They're played
  where they are, never copied. No games, keys or system files come with nativeMons.
- **Saves** belong to the launcher, in `state\3ds\saves`, not to an engine or emulator. Each play hands the save over
  and takes it back afterward, with a backup before and after, so a game can switch between 3DSrecomp+Zakuro and the
  emulators and keep its save (tested both ways on Pokémon Y). A save that fails its check after a session never
  becomes the one in use: it goes to the backups, and your last good save stays.
- **Save slots** work as for the other games, and each slot keeps its own save states.
- **My Collection** counts their Pokédex, and **Move Pokemon** moves box Pokémon in and out of them (Gen 6 and 7,
  with the same rules as the other games). After a move, that slot's save states go to its backups, so loading an
  old one can't undo the move.
- **Not yet:** exporting and importing 3DS saves.
- **Looks:** each game has a drawn theme of its region (Kalos, Hoenn or Alola) in its own colors. Its wallpapers can
  come from your own play: while a 3DS game runs with its window in front, the launcher keeps a few pictures of the
  top screen (leaving out menus and dark or repeated ones) and shows one at random. They stay on your PC.
  **Settings > Wallpapers** switches to the drawn scenes, or gives any game a picture of your own.

## 3DSrecomp+Zakuro

A native engine for all eight games, and smoother than the emulators. [3dsrecomp](https://github.com/fearkov/3dsrecomp)
turns your game's code into C and compiles it into a library, which [Zakuro](https://github.com/fearkov/zakuro) runs
instead of emulating the CPU. It comes with nativeMons, in `engines\zakuro`, and needs a decrypted dump. Zakuro lists
seven of the games as playable; Moon isn't on its list yet, but we tested it ourselves (its code is almost all
Sun's, and it plays the same).

**Building each game, once, on your PC.** No game code is in nativeMons or its downloads.
- When the launcher finds new 3DS games at start-up, it gets them ready behind the splash, then asks once. **Set up
  in the background** fixes any header that needs it and builds the games one by one while you keep using the
  launcher, with a banner showing how far along it is.
- A build takes about 10 to 20 minutes per game on a typical PC, at the lowest priority. **Settings > 3DS build
  speed** picks Fast (all but one processor thread) or Quiet (half). Esc or B stops a build.
- A sister game (X after Y, and so on) builds faster, from the parts they share.
- Pressing Play on a game that's still building says so, and offers the emulator for now.
- Builds stay on your PC, in `state\3ds\recomp`. The first one downloads Zig, the C compiler it uses (about 100 MB,
  checked against a pinned checksum).

**A dump marked encrypted when it isn't.** Some decrypting tools leave the header saying "encrypted" (often for Omega
Ruby, Alpha Sapphire and the Sun and Moon games), and Zakuro and 3dsrecomp refuse it. **Fix 3DS header** (on the
game's page, and offered by the build) checks the game really is decrypted, then changes 2 bytes of the header as
GodMode9 does, keeping the original bytes in `state\3ds\header-backups`. **Undo 3DS header fix** puts them back.
Emulators play the dump the same either way.

**Playing:**
- **Controls** are Fulldex's keys (Settings > Controls lists them, and so does the guide button's overlay), and a
  controller by the letters on its buttons. Hold the right trigger (or Tab) to fast forward, up to 4 times the
  speed. Change either in Zakuro's menu (Esc), under Controls; the launcher sets the controller only once. While the
  guide overlay is open, the controller doesn't reach the game.
- **Screen:** it opens on the launcher's screen, with a big top screen and a small bottom one (F9 or Zakuro's menu
  changes that, and it stays). **Game options** set its volume, fullscreen and Smooth scaling (off means sharp
  pixels). In a window, it's sized to fit the screen.
- **Quick start:** with Quick start on, a game starts from a snapshot of its title screen, made in the background from
  a copy of your save. It's only used while your save is exactly the one it was made from.
- **Save states:** F5 saves, F8 loads. Zakuro's and the emulators' can't load in each other, so each save slot keeps
  both kinds.
- **60 fps (Zakuro)** on a game's page turns on Zakuro's community code for X, Y, Omega Ruby, Sun, Ultra Sun or Ultra
  Moon. It doubles the game's speed too, and only works on the game version it was tested on.
- **Zakuro mods folder** opens the game's mods folder (laid out as for Luma3DS and Azahar). Cheats are in Zakuro's
  menu.

**It gets faster as you play.** Zakuro notes the places in a game's code its build didn't cover. After a session that
found enough of them, the launcher rebuilds the game quietly in the background (lowest priority, half the processor
threads, usually under a minute thanks to the cache), with a small banner showing its progress. Your next play uses
it. Closing the launcher stops a rebuild, and the next session starts it again.

**Sharing helps everyone.** After such a session, the launcher asks whether to share that list: **Yes**, **No**
(asked again after more new findings), or **Never ask again** (**Settings > Ask to share Zakuro findings** turns it
back on, and the game's page can share any time).
- **Yes** sends the list straight to nativeMons' own submission service,
  [submit.hails.app](https://github.com/Hailey-Ross/nativemons-submit), signed by your launcher. No account is
  needed, and it holds only addresses (numbers like `0010022D`): no game code, save or anything about you.
- **Share through GitHub instead** opens a form on this repository and puts your list on the clipboard to paste in.
- Once checked, a list goes into the shared lists, which every launcher downloads at start-up and every few hours,
  with no update needed. A built game whose shared list grew is rebuilt quietly to cover it.

**Known issues:** some cutscenes miss textures or draw the sky wrong. Zakuro's mail and similar extra data aren't
carried between engines (the same goes for the emulators).

## 3DS emulators

Turn on **3DS games** under **Emu** on the home screen (or in Settings), then install one there:
- **AzaharPlus** (recommended): Azahar with extras, and the only one that opens encrypted dumps. Our build leaves out
  its downloader for Nintendo's system files.
- **Azahar**: a backup for when AzaharPlus misbehaves. It needs a decrypted dump; an encrypted one always plays with
  AzaharPlus.

Nothing downloads until you pick one. Each is built from our own fork, checked against a pinned checksum before it's
put in place, and kept in `emulators\`, which a launcher update never touches. Removing one never touches your saves.
When a newer build of your emulator is ready, Play asks whether to update first; **Do not remind me of this update**
stops the question for that one update.

**Controls, as on Fulldex:** every connected controller is mapped by the letters on its buttons, and a second
profile, "Keyboard (Fulldex keys)", has Fulldex's keys. The emulator binds each button to one device, so pick that
profile in its Controls to play on the keyboard. The launcher also sets Vulkan with background shader building (the
first minutes of a game get smoother each time you play), your **Game options** volume, and no "are you sure" when
closing.
