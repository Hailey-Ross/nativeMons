# Settings

[Back to the README](../README.md)

Game options, quick start, Discord status, updates, controls and the rest of the Settings page.

Each engine has its own page of options (**Fulldex**, **Gen1Recomp**), stored where its menus keep them, so changes show up on both sides.

**Game options** set volume, music, sound effects, fullscreen, widescreen, smooth scaling, text speed, battle scene, battle style and sound for every game, and a game's own page can override them. Each one is left alone until you choose it, and it applies when a game starts. Volumes go in steps of 5%, chosen from a list; an engine with coarser steps uses the nearest one it has (Fulldex and the GBA player go in tens). Not every engine has every setting: the page says which. Options kept in the save aren't available on Generation 5 and later yet.

**Emu** (also under **3DS games** in Settings) turns 3DS games on and installs, updates or removes the 3DS emulators.

**Quick start** opens games straight at their title screen, past the copyright and intro. On Gen1Recomp (Game Boy, Game Boy Color and GBA games) and nativeplat (Diamond, Pearl, Platinum), the last game you played waits at its title out of sight, silent and paused, while the menu is open, so Play shows it at once; it's used only if its save and options are still what it read. DS games on Fulldex and 3DS games on 3DSrecomp+Zakuro start from a snapshot of their title screen, made in the background from a copy of your save; it's used only while your save is exactly the one it was made from, and the game's clock is set to now. Either is thrown away and made again whenever something it loaded could have changed. Turn it off to start games with their intro. GBA games on Fulldex, the Platinum PC port and Infinite Fusion start as before.

**Discord status** shows what you're playing while a game runs: "Playing nativeMons", the game and a timer, with a button others can use to find nativeMons on GitHub, through the Discord app on your PC. It clears when the game closes, and does nothing when Discord isn't open. Gen1Recomp's own Discord status is switched off for games the launcher starts, so only this one shows. Turn it off in Settings.

**Updates** checks this project's releases when the launcher starts and every 6 hours while it's open. When a new build is ready, a banner appears for a few seconds and Settings gets a badge; nothing downloads until you choose to. The same page switches between **Stable** and **Dev**, and names what you have: a stable build as "version 0.0.5, build N", a dev build as "build N, after version 0.0.5". Updating replaces the launcher and every engine together from one checked download, and changes only the files that differ. Your saves, ROM copies, wallpapers, Pokédex data, settings and anything in `games\` stay as they are. A copy built from source reads as a local build, and any release counts as newer until you build again.

**Controls** lists the keys each engine uses while you play (Fulldex, Gen1Recomp, nativeplat, the Platinum PC port and Infinite Fusion): saving, fast forward, snapshots, screenshots and where to change them.

While a game runs, the controller's guide button (Xbox, or the PS button on a DualShock 4 or DualSense) opens an overlay over it: resume, that game's controls, or close the game.

**Screenshots** opens the folders where nativeplat (F12) and Infinite Fusion (F8) put the screenshots you take. Fulldex and Gen1Recomp have no screenshot key; Windows + Shift + S works in any game.

**Import saves**, **Export saves**, **Download Pokedex**, **Credits** and **Last session** (how the last game ended) are here too.
