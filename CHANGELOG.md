# Changes

Each notable stable release raises the version, and its section here becomes its release notes. Minor updates, such as engine updates, keep the number. Dev builds never change it.

## Next

- Fulldex 1.7.0 for the DS games. The Hub shows your current boxes, party and Pokédex again. A full-size DS game from another region (such as a Spanish HeartGold) is the game itself, not a ROM hack. Also new: a much bigger Guide for Black, HeartGold, SoulSilver and Platinum, the pause menu and Pokédex in seven languages, each Pokémon's evolution family, Nintendo WFC through a community server, and Auto Save State (both off to start). Settings work with taps, clicks and the mouse wheel. In HeartGold and SoulSilver battles, the Yes and No buttons no longer cover your HP box.
- Engines brought up to date with upstream: Gen1Recomp, Fulldex's source for our GBA player, nativeplat, 3DSrecomp+Zakuro and both 3DS emulators.
- Platinum PC port v0.2.3: a gamepad that disconnects can be reconnected, the left stick works as the D-pad, and the overlay menu can be opened and used entirely from a gamepad.
- Move Pokemon: OpenHome's converter v1.19.2. Gen 1 and 2 Pokemon are no longer given the same personality value (which made them look like clones), and nicknames and trainer names are cut to fit games with shorter limits.

## 0.0.8a

- Fulldex 15 November for the DS games: FullDex Nav, a watch menu on L3 (or N) with Fly, Party, Open PC, Repel and an encounter counter; a fuller Pokédex in its pause menu; Fly from its own menu; and the right stick to zoom out and look around. Its new tidy folders don't change anything for nativeMons: your saves stay where they were.
- Engines brought up to date with upstream: Gen1Recomp, Fulldex's source for our GBA player, nativeplat, 3DSrecomp+Zakuro, and both 3DS emulators.
- My Collection looks more like the games: a Poké Ball by each Pokémon you've caught, dashes for names you haven't seen yet, and the shiny picture for one caught shiny. Each region has its own colors and pattern, with a bar of how much of it you've caught.
- A caught Pokémon shows a badge for each game you have it in, in that game's colors, with a star where it's shiny.
- Pokémon you got by trading, or moved in with Move Pokemon, count as caught even where the game's Pokédex never recorded them.
- Wide layout, for landscape screens: the menu as a column on the left, with the chosen option shown large beside it (a game's title screen plays there). It turns on by itself on short screens, such as a Steam Deck's, and has a switch in Settings where it fits.
- Menu options are separate rounded buttons, and the volume sliders are easier to read and use.
- Links in Credits open in your browser.
- Linux and Steam Deck, early and not ready for everyone: `nativemons.sh` runs the launcher under Proton or Wine. See `docs/linux.md`.

## 0.0.8

- Share Zakuro findings sends your list straight to nativeMons' own submission service, submit.hails.app: no account needed, signed by your launcher, and checked before anything uses it. Sharing through a GitHub form is still there as the other way, with the list on your clipboard to paste.
- The shared Zakuro lists now cover the start of every game, not only its title screen: about four times as many places for each of the eight games, so first builds run smoother from the start.
- The README is shorter, with each topic (3DS games, saves, mods, settings and more) on a page of its own in `docs`, which now comes with the download.

## 0.0.7a

- Quick start for 3DSrecomp+Zakuro: a 3DS game starts from a snapshot of its title screen, made in the background from a copy of your save and used only while your save is exactly the one it was made from.
- Shared Zakuro findings now download on their own, at start-up and every few hours, from their own branch of our Zakuro fork; a built game whose shared list grew is rebuilt quietly to cover it. Shared lists no longer wait for a Zakuro update.

## 0.0.7

- 3DSrecomp+Zakuro, a native engine for X, Y, Omega Ruby, Alpha Sapphire, Sun, Moon, Ultra Sun and Ultra Moon: each game's code is recompiled once on your PC from your own decrypted dump, then runs in Zakuro. The launcher asks before each build (about 10 to 20 minutes per game, at the lowest priority, Fast or Quiet in Settings), and saves move freely between it and the emulators.
- 3DSrecomp+Zakuro: Fulldex's controls and a controller by its letters, the launcher's volume, fullscreen and smooth scaling, save states in each save slot, a 60 fps switch and a mods folder on the game's page, and its own Controls sheet. It opens on the launcher's screen with the big top screen, fast-forwards (up to 4x) on the right trigger, and ignores the controller behind the guide overlay. After a session that found code its build missed, the game is rebuilt quietly in the background, with a progress banner, and the launcher asks whether to share those findings for everyone's builds (yes, no, or never ask again).
- Move Pokemon: OpenHome's converter v1.19.1. A Pokemon moved back to Gen 3 from Gen 4 or 5 now shows as met in a trade, not at an unrelated place.
- New 3DS games found at start-up are set up together: their headers fixed if needed (asked first) and built one by one in the background, with a progress banner, while you use the launcher.
- Fix 3DS header: a decrypted 3DS dump whose header still says encrypted can be fixed from its page (2 bytes, with a backup and Undo), so 3DSrecomp+Zakuro takes it.
- Gen1Recomp: the latest upstream updates, with story script, battle and Gen 2 fixes, plus mail and Quest Log fixes for FireRed and LeafGreen. Red, Blue, Yellow, Gold, Silver and Crystal are prepared again from your ROM the next time they start.
- An encrypted 3DS dump always plays with AzaharPlus, even when Azahar is the emulator picked: Azahar only opens decrypted dumps. A decrypted dump of the same game is still used with Azahar.

## 0.0.6

- Fulldex 12 November: the nickname question after a catch no longer covers your Pokémon, HeartGold and SoulSilver radio text stays in view in widescreen, DS music keeps its normal tempo and pitch during fast forward, and many smaller fixes.
- Gen1Recomp: the latest upstream updates, including Crystal sound fixes and, in the Gen 3 games, trainer party and Continue fixes, the Bag working in Battle Pyramid battles, the gift Castform holding its Mystic Water, and one controller press no longer skipping two menu options.
- Wallpapers: a random town each start, from every town and city of the game, for every game family.
- Closing the launcher also closes the games it started in the background, and nothing new starts while it closes.
- 3DS games, optional and in progress: X, Y, Omega Ruby, Alpha Sapphire, Sun, Moon, Ultra Sun and Ultra Moon through AzaharPlus (recommended) or Azahar as a backup, installed from Emu on the home screen with its own install animation. Saves stay with the launcher, backed up before and after each play, with save slots and per-slot save states.
- A new Collection icon: a phone Pokédex.
- A guided first-time setup: welcome, your games, 3DS games, wallpapers and done, sliding from step to step.
- The launcher's own folder and file browser, with its own little icons, in place of Windows' dialogs for ROM folders, ROM files and pictures.
- Wallpapers: drawn scenes or pictures from your own play, asked once and changeable in Settings, plus a picture of your own for any game, picked from your PC or found online (fan art from wallhaven.cc, free-license art from Openverse). Sun and Moon now differ, and the Ultra versions glow in a complementary color.
- The mouse only picks a row when it really moves, so a still cursor no longer takes the selection from the keys.
- GBA games whose ROM path has an accent (such as "Pokémon - LeafGreen Version") start again: the player gets a plain path.
- Platinum PC port v0.2.2: importing ROMs from other regions. Its saves were checked: Continue loads a DS save, and the save it writes passes the checks.
- A game's Engine opens a list of its engines, with the one in use marked, instead of switching to the next one.
- Volume, music and sound effects are sliders in Game options: left and right change them by 5.
- The guide button (Xbox, or PlayStation's PS button) opens an overlay while a game runs: resume, its controls, or close the game.
- 3DS emulators control like Fulldex: a controller by the letters on its buttons, and Fulldex's keys as a second profile, "Keyboard (Fulldex keys)".

## 0.0.5

- First numbered release. From here the version goes up only with a notable stable release; minor ones, such as engine updates, keep the number.
- The launcher and updater show a stable build as "version X, build N" and a dev build as "build N, after version X".
- Fulldex 8 November: a Manage Saves page for each game (save states, backups, export and import), ROM hacks with saves of their own, patch files put on your own clean ROM, and fast forward that stays on.
- Platinum PC port v0.2.1: fixes a crash in the Battle Hall. Its saves stay in the DS format.
- Gen1Recomp: the latest upstream updates.
