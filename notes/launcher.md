# Launcher

`launcher/NativeMonsLauncher.cs`: one menu for native Pokemon ports on PC, for a controller, keyboard or mouse (D-pad or left stick to move, A to select, B to go back; arrows, Enter and Esc work too).

The window, splash, tiles and detail panel come from the steam launcher on the Skate 3 Rust engine's hails-additions branch (github.com/Hailey-Ross/rusty-trucks, tools/steam-launcher, GPL-3.0). The games run as separate programs, so nothing from any engine is built into the launcher.

It builds with the C# compiler that ships with Windows (`launcher/build.bat`), which means C# 5: no string interpolation and no `?.`.

## Command line

| Flags | What it does |
| --- | --- |
| `--game <id>` (or `--play <id>`) | Starts that game and closes afterwards (shortcuts, Steam). |
| `--slot <save>` | With `--game`: which of the game's saves to use, by number or name. |
| `--launcher` | With `--game`: opens the menu with that game chosen instead of starting it. |
| `--snapshot <out.png> <screen>[:<game id>] [<w>x<h>]` | Draws one screen to a picture and exits. A size wider than it is tall shows the tablet. |
| `--export <id> plain\|checkpoint <folder> [backup]` | Exports a save. The result goes in `<folder>\export-result.txt`. |
| `--sync <id> <result file> [fulldex\|gen1]` | Shares one game's save between its engines. |
| `--add-roms <result file> <path>...` | Adds folders, ROMs and zips the way dropping them on the window does, and says what each ROM is. |
| `--add-pc <result file> folder\|zip <path> [program number]` | Adds a PC game. With several programs and no number, it lists them and adds nothing. |
| `--install-pc <id> folder\|zip\|download <result file>` | Installs a PC game the way its page does, then makes its pictures. |
| `--install-mod <zip> <result file> [mod id]` | Installs a Gen1Recomp mod. With an id, it only installs the zip if it holds that mod. |
| `--install-index-mod <id> <mod id> <result file>` | Find mods' Install. |
| `--mod-clashes <id> <result file>` / `--mod-deps <id> <result file>` | What the Mods page shows for clashes and missing mods. |
| `--roms`, `--update`, `--saves` | For tests: the scan's findings, an update with no window, and the save actions (`list`, `new`, `use`, `copy`, `delete`, `recover`). |

Snapshot screens include `main`, `games`, `game`, `mods`, `folders`, `credits`, `settings`, `controls`, `controls-<n>`, `screenshots`, `job`, `splash`, `sprites`, `updatebanner`, `clashbanner`, `clashes:<game>`, `clashon:<game>,<mod>`, `deps:<game>`, `dep:<game>,<mod>` and `boxes`, `boxheld`, `boxchoose:<top>,<bottom>`. A banner preview with nothing real to show uses a sample instead.

There's one launcher per folder: a second start writes `state\request.txt` and posts a message to the window that's already open, which then does what was asked. A copy of nativeMons in another folder is a separate launcher, with its own games and saves.

## Saves

- **Backups (SaveGuard):** every save that's about to change is copied first and the copy is checked byte for byte. Only checked copies are kept: the newest 30 per save, and never the last healthy one.
- **Sharing:** a game that both Fulldex and Gen1Recomp can run shares one save, the cartridge file Fulldex uses. A sync never throws and never replaces a save without a verified backup and a proven result. If it fails, both saves stay as they were and it's tried again next time.
- **Several saves per game:** the save in use sits where the engines expect it, and the others are parked in `state\save-sets\<game>\<number>`. Nothing is shared during a switch: the save being left is parked and proven first, so a failure can put everything back. A switch cut short by a crash is undone at the next start.
- **Platinum PC port:** its `save.bin` holds the same data as a DS save, but its 64-bit build pads some structures to 8 bytes: the s64 times in GameTime and SystemData, the BoxPokemon (u64 ribbons) in each Daycare slot, and PCBoxes. A save converts either way by adding or removing that padding and redoing each block's footer. That was up to v0.1.13. From v0.2.0 the port uses the DS format itself (tested 6 November: a DS save continues, and the save it writes passes mon-bridge check). The save now goes in as it is and comes back checked; only an old padded `save.bin` left from v0.1.13 is converted back (`PlatinumSave.IsOldPort`, the port's own test: a footer signature at 0xCF38). The port asks to overwrite on every in-game save; that's normal.
- **nativeplat:** it keeps raw cartridge saves in named slots under `userdata` (with `portable.txt` beside it). Each game gets one slot, which is treated like the port's `save.bin`.

## Gen1Recomp

- **Data folder:** saves, options, mods, exports and its ROM cache live in `state\gen1recomp`, passed in with `NATIVEMONS_DATA_DIR` (portable mode in its SaveData), so they move with the launcher's folder. Before 2026-11-03 they were in `%APPDATA%\LOVE\nativemons`. That folder is copied over once, every file is checked, and the old folder is left alone as a backup. LOVE still keeps a folder under `%APPDATA%\LOVE` for the few caches that ignore portable mode, under a different name, so the old folder is never read or changed again.
- **Accented paths:** Gen1Recomp reads paths from the environment in the system code page, which garbles names like "Pokémon". Such a file goes in through a temporary copy with a plain name (PlainPath).
- **Preparing games:** Gen1Recomp builds a game's data from its ROM before the first play, which takes up to a minute for a GBA game. The launcher prepares every game behind the splash, so Play starts straight away. The first game runs alone, because an older install may move its Red data on the first import. After that, three run at once, biggest ROMs first. Most of the load isn't the engine but Windows Defender scanning about 60,000 small files. Running one at a time only spreads that out: 70 seconds instead of 49.
- **Mod defaults:** Gen1Recomp's own migration wrote every installed mod's default out as if the player had chosen it. Mods the fork now keeps off by default (`OffByDefault`) are switched off once, and after that the player's choice stands.
- **Discord:** the launcher shows its own Discord status, so Gen1Recomp's is turned off (`POKEPORT_NO_DISCORD`).

## Mods

- **Community index:** github.com/bryanthaboi/gen1recomp-mod-index publishes a feed (`data/index.json`), which the launcher reads with its own code and caches for six hours. Installing from the index works like installing a .zip. Being listed isn't a check of a mod, and the launcher says so before an install. For a GitHub repo, the release's zip is `<id>-<version>.zip` if there is one, otherwise any .zip, the same order the engine's updater uses (`src/mods/ModUpdate.lua`).
- **Dependencies:** a spec is `id@range#repo`. The Needs other mods page lists missing mods and wrong versions, required ones first, leaving out optional ones the player turned down.
- **Clashes:** a mod can say it doesn't work with another (`conflicts` or `incompatible` in its manifest.json). While both are on, Gen1Recomp skips the mod that made the claim (`Loader:_enforceConflicts`) without telling the player, so the launcher does. A newly installed mod is switched on in every game it runs, unless it clashes there with a mod that's already on.
- **Clash banner:** a red banner at the top that stays up while any mods clash, except on the Mods pages, which have their own Clashes row. Clicking it opens the clashing game's list. Clashes are read in the background at start, after a game closes and when the player leaves the Mods pages, because each read starts the engine.

## PC games

- **Infinite Fusion:** an RPG Maker XP game on mkxp-z. Its folder is a git checkout of infinitefusion/infinitefusion-e18, branch `releases`. Update does what the game's own `INSTALL_OR_UPDATE.bat` does: a shallow fetch and a hard reset. An install goes into a folder beside the final one and only replaces it once it's complete.
- **Imported or existing:** an imported copy lives in the launcher's games folder, is updated by the launcher, and keeps its saves in the launcher's state folder. An existing installation is the player's own: it runs where it is, is never changed, and keeps its saves where the game puts them (`%APPDATA%\infinitefusion`). `state\pc-<id>.txt` names an existing one.
- **Saves beside the launcher:** mkxp-z puts saves under `%APPDATA%` with no setting to change that. An imported copy gets an `mkxp.json` naming a preload script that points `System.data_directory` at the launcher's folder, but only when the launcher passes `NATIVEMONS_SAVE_DIR`. `Game.exe` started by hand behaves as usual. Git doesn't track either file, so the game's own update leaves them in place.
- **Pokedex:** read from the game's saves through a read-only Ruby Marshal 4.8 reader that stops on anything it doesn't know. `Player::Pokedex` keeps owned and seen by the game's own species numbers. Those become National Dex numbers by species name, from `Data\species.dat`, which the game XORs with a fixed key in its Encryption module.
- **Added games (CustomGames):** any self-contained game with its own .exe, kept in `state\pc-games.json`. A game can come from its folder (run in place), a .zip (unpacked into `games\<id>`, never outside it) or its program. The likeliest program comes first, leaving out installers, uninstallers, crash reporters and runtimes. These games get none of the extras, like a captured title, a wallpaper or save handling.

## ROMs and romhacks

- A romhack becomes a game of its own, listed after its base game. A hack keeps its base game's code in its header, so it's sorted there first. It's told apart by its contents alone: it isn't one of the clean dumps an engine accepts. The file name only names it. Hacks run on Fulldex only.
- Known hacks are listed in games.json (`hacks`) by the SHA-1 of the patched ROM, and get that name and id whatever the file is called. Hashes come from the author or a reputable database (TASVideos). A web patcher gives the same output for everyone who uses the same clean base, so one hash covers a version.
- nativeplat's own import refuses Platinum Rev 0. pret's build files show the revisions differ only in the header's revision byte and one GTS overlay, which nativeplat doesn't take from the ROM, so Rev 0 is accepted.
- An .nds ROM whose secure area is still encrypted won't load without a DS BIOS. It's patched on a copy, and the player's file isn't touched.

## Pictures

- **Title screens (GameArt):** captured from the player's own ROM on their PC, by running the game with no window. None of it ships: it lives in `state\game-art`, and a game without a capture keeps its drawn scene. For DS games, the widescreen view shows both screens side by side. The logo is the half that looks the same in every picture.
- **GBA romhacks:** Gen1Recomp only runs clean copies, so a hack's title screen comes from mGBA's libretro core (`tools/mgba`), run by the launcher itself (`--gba-art`) in a process of its own, with no window or sound, on a copy of the ROM. START at frame 1000 skips the opening, and the pictures start at 1500: every FireRed and Emerald hack tried was on its title screen by 1460. Until it has its own, a hack shows its base game's pictures. Its town wallpaper is always its base game's.
- **Game Boy and GBA:** Gen1Recomp shows the title screen in a hidden window with the sound off. A game that isn't prepared yet is prepared under a throwaway identity and removed afterwards.
- **Towns:** nativeplat's test mode, which uses its own saves. Diamond and Pearl play a scripted new game (from gen4recomp's `tests/dp`) until they can walk, then jump to the town. Platinum's save lab makes a save standing there. Generation 5 games use their own opening, which flies over the home town at a fixed frame set in games.json.
- **Spinner:** a Poke Ball that turns once every 2 seconds while its top half runs through the rainbow, after the "Updating" page on pogo.hails.app.

## Quick start

- The last game waits at its title screen, out of sight, silent and stopped, while the menu is up, so Play shows it at once. Gen1Recomp's side is `src/nativemons/QuickStart.lua`, and nativeplat's is `NP_QUICK_START` in the gen4recomp fork. Anything that could change what the game loaded throws it away.
- Fulldex DS games start from a snapshot of their title screen, made in the background from a copy of the save. Loading a snapshot writes its cartridge save to the save file, so a snapshot is only used while the save, ROM, Fulldex and settings all match the ones it was made from.

## Fulldex

- After a GBA game ends, Fulldex's published build opens its own game list instead of exiting, and has no option to change that. When the game's window gives way to the list, the game has already saved, so the launcher closes Fulldex.
- Fulldex only starts games it knows on its own. A romhack is set as the last game played, so the list opens on it, and it's started with one key press. That press must never answer an update prompt, so update checks point at an address that can't be reached.

## Settings

Settings hold a default for all games, and each game can follow it or choose its own. "own" leaves things as they are. Most settings are kept inside each game's save; engine settings go into the engine's own settings when a game starts.

## Window

- **Tablet mode:** for Move Pokemon, the device opens out sideways into a tablet with one wide screen, so two games' boxes sit side by side, and folds back when the boxes close. The window takes its tablet size once, when the unfold starts, and gives it back once, when the fold ends. In between, only the device drawn inside it changes, so the window never resizes while it animates (resizing made it judder).
- **Update banner:** slides in at the top and leaves by itself after 8 seconds. It takes no keys or focus and installs nothing. Clicking it opens Updates; closing it hides that build for good.
- **Updates:** come whole from this project's releases. The state folder and the player's files are never touched.

## Discord

While a game runs, the player's Discord status shows which one. The launcher speaks Discord's local pipe protocol (a handshake, then `SET_ACTIVITY`) with no library, and closing the pipe clears the status. Nothing happens when Discord isn't running or when the player turns it off in Settings.
