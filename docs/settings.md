# Settings

[Back to the README](../README.md)

Game options, quick start, Discord status, updates, controls, and everything else on the Settings page.

## Game options

**Game options** set volume, music, sound effects, fullscreen, widescreen, smooth scaling, text speed, battle scene,
battle style and sound for every game, and a game's own page can override any of them.
- Each option is left alone until you choose it, and applies when a game starts.
- Volumes go in steps of 5%, chosen from a list. An engine with coarser steps uses its nearest one (Fulldex and the
  GBA player go in tens).
- Not every engine has every setting, and the page says which do. Options kept in the save aren't available on
  Generation 5 and later yet.

Each engine also has its own page of options (**Fulldex**, **Gen1Recomp**), stored where its own menus keep them, so
a change shows up in both places.

## Quick start

**Quick start** opens games straight at their title screen, past the copyright and intro. Turn it off to see the
intro again.
- **Gen1Recomp** (Game Boy, Game Boy Color and GBA games) and **nativeplat** (Diamond, Pearl, Platinum): while the
  menu is open, the last game you played waits at its title out of sight, silent and paused, so Play shows it at
  once. It's only used if its save and options are still what it read.
- **DS games on Fulldex** and **3DS games on 3DSrecomp+Zakuro**: the game starts from a snapshot of its title screen,
  made in the background from a copy of your save. It's only used while your save is exactly the one it was made
  from, and the game's clock is set to now.
- Either one is thrown away and made again whenever something it loaded could have changed.
- GBA games on Fulldex, the Platinum PC port and Infinite Fusion start as usual.

## Updates

**Updates** checks for new releases when the launcher starts and every 6 hours while it's open. When a new build is
ready, a banner shows for a few seconds and Settings gets a badge. Nothing downloads until you choose to.
- **Stable or Dev:** the same page switches between them, and names what you have: a stable build as "version 0.0.8d,
  build N", a dev build as "build N, after version 0.0.8d".
- **One checked download** replaces the launcher and every engine together, changing only the files that differ.
  Your saves, ROM copies, wallpapers, Pokédex data, settings and anything in `games\` stay as they are.
- A copy built from source shows as a local build, and any release counts as newer until you build again.

## Discord status

While a game runs, your Discord status shows "Playing nativeMons", the game and a timer, with a button others can use
to find nativeMons on GitHub. It works through the Discord app on your PC, clears when the game closes, and does
nothing when Discord isn't open. Gen1Recomp's own Discord status is switched off for games the launcher starts, so
only one shows. Turn it off in Settings.

## Controls and screenshots

- **Controls** lists the keys each engine uses while you play (Fulldex, Gen1Recomp, nativeplat, 3DSrecomp+Zakuro,
  the Platinum PC port and Infinite Fusion): saving, fast forward, snapshots, screenshots, and where to change them.
- **The guide button** (Xbox, or the PS button on a DualShock 4 or DualSense) opens an overlay over a running game:
  resume, that game's controls, or close the game.
- **Screenshots** opens the folders where nativeplat (F12) and Infinite Fusion (F8) put your screenshots. Fulldex and
  Gen1Recomp have no screenshot key, but Windows + Shift + S works in any game.

## Also here

- **Emu** (also under **3DS games**): turns 3DS games on, and installs, updates or removes the 3DS emulators.
- **Import saves** and **Export saves** (see [Saves](saves.md)).
- **Download Pokedex**, for offline pictures and entries (see [My Collection](pokemon.md#pokémon-pictures)).
- **Wide layout**, on landscape screens at least 1000 pixels across: the menu as a column on the left, with the
  chosen option shown large beside it. It changes the window right away. Left alone, the launcher picks the wide layout
  on short screens, such as a Steam Deck's 1280x800.
- **Credits**, with links that open in your browser, and **Last session**: how the last game ended.
