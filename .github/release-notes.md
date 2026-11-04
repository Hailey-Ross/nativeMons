## Setup

**What you need:** a Windows 10 or 11 PC, and your own Pokémon game files (ROMs). No games come with nativeMons.

1. **Download** the zip: click **Download Windows ZIP** above.
2. **Unzip it.** Right-click the downloaded file and choose **Extract All...**, then **Extract**. Put the folder somewhere easy to find, such as your Documents or Desktop (not inside Program Files).
3. **Open it.** In the new folder, double-click **NativeMonsLauncher**.
   - If Windows shows **"Windows protected your PC"**, click **More info**, then **Run anyway**. This appears because nativeMons is a free project without a paid signing certificate.
4. **Add your games.** On the home screen, choose **ROMs**, then **Add a folder**, and pick the folder your game files are in. Or just drag that folder onto the nativeMons window.
   - It finds Game Boy, Game Boy Color, Game Boy Advance and DS games (`.gb`, `.gbc`, `.gba`, `.nds`, and `.zip` files holding them). The file names don't matter.
5. **Wait about a minute the first time.** nativeMons gets your games ready behind its loading screen. After that it opens in about a second.
6. **Play.** Choose **Games**, pick a game, and press **Play**.

**Already have saves from another emulator?** Choose **Settings**, then **Import saves**. Your original save files aren't changed.

**A DS game that won't start?** Some DS ROMs need a small fix first. nativeMons offers it when it finds one, and if your PC doesn't have Python (which the fix needs), it can download it for you.

## Games

| Engine | Games |
|---|---|
| **Gen1Recomp** (native, with mods) | Red, Blue, Yellow, Gold, Silver, Crystal, Ruby, Sapphire, FireRed, LeafGreen, Emerald |
| **Fulldex** (widescreen, 60 fps) | FireRed, LeafGreen, Emerald, HeartGold, SoulSilver, Platinum, Black, White, Black 2, White 2 |
| **Platinum PC port** (native, optional) | Platinum |
| **nativeplat** (native) | Diamond, Pearl, Platinum (optional) |

Where both engines have a game, it starts in Fulldex. Switch on that game's page. The engines share one save, if your ROM works with both. The Platinum PC port's download has only the program: its game files are unpacked from your own ROM, on your PC.

## Controls

- **D-pad or left stick:** move. **A:** select. **B:** back.
- **Keyboard:** arrow keys or WASD, Enter, Esc.
- **Mouse:** hover to select, click to choose, right-click to go back. The D-pad and buttons on the device can be clicked too, and dragging its shell moves the window.
- **In a game:** **Settings > Controls** lists each engine's keys, such as saving, fast forward, snapshots and screenshots. **Settings > Screenshots** opens the folders they're saved in.

## What's in it

- **One launcher** for every game, shaped like a phone-sized Pokédex that takes on each version's colours, with a phone-style screen. A game's intro plays in its Play button, and its home screen shows one of its towns as the wallpaper, both captured from your own ROM on your PC.
- **Game options for every game:** volume, music, sound effects, fullscreen, widescreen, smooth scaling, text speed, battle scene, battle style and sound, set once in Settings or per game. Each is left alone until you choose. Options kept in the save aren't available on Black, White, Black 2 and White 2 yet.
- **Several saves per game**, each with a name you choose. Switch, copy, rename or delete them from a game's page, and import saves with their trainer, play time, badges and party shown first.
- **My Collection:** a Pokédex with National and regional lists and full entries for what you've caught, including the games you have it in and any shinies. Entries come from PokéAPI. It counts each Pokémon once across all your saves, against the entries your games cover. ROM hacks and fan games never count toward it; they have a section of their own, where Infinite Fusion's Pokédex is read from its saves, fusions included.
- **Move Pokémon** as boxes, like Pokémon HOME: the Pokédex unfolds into a tablet with two games' boxes side by side, drag a Pokémon from one to an empty slot in the other, or with a controller press A to pick it up (the cursor jumps to the other game), X to switch games and LB/RB to change boxes. If a moved Pokémon isn't legal in its new game, the message says what was found. Between games, using PKHeX.Core, with both saves backed up first. If attacks or a held item stand in the way, you choose what to delete or change, then retry. A Pokémon copied by hand keeps its original data, which comes back when it returns to its own generation.
- **One save across engines** on FireRed, LeafGreen, Emerald and Platinum, with every conversion checked before it's used.
- **Portable:** saves, settings and mods stay in the launcher's folder, for every engine (an existing Infinite Fusion installation keeps its own). Gen1Recomp's saves come over from `%APPDATA%\LOVE\nativemons` on the first start, every file checked; that takes a few minutes once, and the old folder is left as a backup.
- **Save backups:** checked copies before anything changes, a corruption check after each game, and restore from a game's page.
- **Import and export saves:** plain `.sav` files for emulators and flash carts, and Checkpoint folders for the 3DS.
- **Open in PKHeX or OpenHome:** a game's page opens your own copy of either, ready for that game's save.
- **Settings** for both engines in one place, including **Download Pokedex** for offline play.
- **Updates:** the launcher checks this project's releases, shows a short banner when a build is ready, and can update itself and every engine from one checked download, leaving your saves and settings alone. Choose **Stable** or **Dev** on the Updates page.
- **Several copies of a game:** if your folders hold more than one version, a game's page lets you choose which to use. ROM hacks are games of their own, named after their file (rename them on their page), on Fulldex with their own saves.
- **Pokémon Infinite Fusion:** from Games, import it into the launcher (from your own copy, its zip or its GitHub), which then keeps the game and its saves in its own folders and updates it; your existing saves are copied in. Or add your existing installation, which runs where it is with its saves where the game keeps them. Saves are backed up before each session either way.
- **Add a PC game:** any other self-contained game with its own program can be added from Games: point to its folder (run where it is), give its `.zip` (unpacked into the launcher), or choose its `.exe`. No extras: the launcher starts it and leaves its saves to the game.
- **Quick start:** games open straight at their title screen. On Gen1Recomp and nativeplat the last game you played waits there while the menu is open, so Play starts it at once; DS games on Fulldex start from a title-screen snapshot, used only while your save is exactly the one it was made from. Turn it off in Settings.
- **Discord status:** while you play, Discord shows "Playing nativeMons" with the game, a play timer and a button to nativeMons on GitHub. It clears when the game closes. Turn it off in Settings.
- **Drop files on the window:** ROM folders, single ROMs, `.zip` files of ROMs (copied out, the zip unchanged) and `.sav` saves. A page says what each ROM turned out to be.
- **Shortcuts:** `NativeMonsLauncher.exe --game <id> [--slot <save>] [--launcher]` starts a game, optionally on a chosen save, or opens its page. If the launcher is already open, it takes the request.
- **Mods** for Gen1Recomp games, from a `.zip` or straight from the Gen1Recomp community mod index (**Find mods**), with the mods they need installed from where you choose and the ones they clash with pointed out, and **hails.nds-Patcher** for `.nds` ROMs that won't load (no Python 3 on the PC? the launcher can fetch python.org's own package just for it). When mods clash, a red banner says so until you settle it.
- **ROM hacks known by their hash:** Rocket Edition, BugLime, Unbound, Adventure Red Chapter, FireRed Evolved, Emerald Imperium, Elite Redux, True Black and more get their proper name whatever their file is called. A GBA hack's title screen plays in its Play button, captured with mGBA; until then, and for its wallpaper, it shows its base game's.
- **Our own GBA player:** GBA games and hacks now play in `engines\gbawide`, built from our Fulldex fork: FireRed, LeafGreen and Emerald in widescreen (the window's shape, or a ratio you choose), hacks at 3:2, with a pause menu (Esc, Guide, or Start and Select) for widescreen, scaling, volume, FPS and fast forward, and RetroAchievements with the account you log in to in Fulldex. Romhacks and versions our patches don't cover (Ruby, Sapphire, other languages, Unbound, Radical Red, Rocket Edition, BugLime, Adventure Red, FireRed Evolved, Elite Redux and more) are widened by PokeDaisy's EXPAND. Based on PokeDaisy by Lidor Itzhari - https://github.com/lidor30/pokedaisy. Ultra Violet plays too, from its `.gba` or its `.ips` (with your FireRed 1.0), saved as `ultraviolet.sav` as in Fulldex. Saves stay where Fulldex keeps them. DS games still play in Fulldex.
- **Games** lists the original games, then ROM hacks and fan games, each section folding away with A, or Left and Right.
- **New icons** on the home screen and through the menus, drawn in the games' own style. The highlighted one moves, a highlighted Play waves hello now and then, opening an app grows its icon to fill the screen, and closing powers the launcher down like an old device before it drops away.

## Testing and reports

- One problem per report if you can, with the game, what you did and what you expected. A screenshot helps a lot.
- **Back up your saves** before moving Pokémon or restoring to a device.
- Report here: [**New issue**](https://github.com/Hailey-Ross/nativeMons/issues/new). Don't attach ROMs.

**Please test:** a controller through every screen, moving Pokémon between your own saves, and restoring an exported save on a real 3DS.

## Known issues

- **Shared saves:** a save with Gen1Recomp mod content may not pass the conversion check, in which case the two stay separate and the launcher tells you. Quicksaves and save states aren't shared, only in-game saves. Not yet played back and forth by hand.
- **Cross-generation moves:** moves with no real transfer between the games, such as Gen 5 to Gen 3, use OpenHome's converter. Form, ball and origin may change, and PKHeX may flag the result as not legal. Try one in battle before relying on it.
- **Platinum PC port:** experimental, from its own project. Sharing a save with Fulldex has only been tried on early saves, so keep a backup. Report problems here first.
- **Intros and town wallpapers:** intros need a game prepared or a ROM the engine accepts. Town wallpapers cover the Game Boy, Game Boy Color and GBA games with a ROM Gen1Recomp accepts, Diamond, Pearl and Platinum with one nativeplat accepts, and HeartGold, SoulSilver, Black, White, Black 2 and White 2 once decrypted. Black, White, Black 2 and White 2 use a shot from the game's opening, closer in than the others. They're captured in the background the first time, which takes a few minutes per game.
- **3DS export:** only part of this has been tried on a real 3DS. Game Boy saves from another emulator can't be exported for Virtual Console, because they carry that emulator's clock.
- **Gen1Recomp launcher features** with no replacement yet: carts, box tools, skins and mod asset-pack import. Online play is removed.
- **My Collection** counts National Dex entries, so the total is the largest Pokédex among your games.

## Credits

Parts of Gen1Recomp inspired this project and helped with research along the way. For the Gen1Recomp engine included here: Based on the Pokemon Gen 1 Recompilation Project by BOIS CLUB GAMES, LLC (https://github.com/bryanthaboi/gen1recomp).

The GBA player follows Pokemon GBA Widescreen by mamesj and uses [mGBA](https://mgba.io) by endrift and contributors. [Pokémon Fulldex](https://github.com/mamesj/pokemon-fulldex) by mamesj, a fork of [ssrecomp](https://github.com/ClickingMaster/ssrecomp), using [melonDS](https://melonds.kuribo64.net) and [mGBA](https://mgba.io). [Pokémon Platinum PC port](https://github.com/cybervisi0n/pokeplatinum) by cybervisi0n and contributors, using [libntr](https://github.com/cybervisi0n/libntr), from the [pret](https://github.com/pret/pokeplatinum) decompilation; they aren't affiliated with nativeMons. [nativeplat](https://github.com/sohamb117/gen4recomp) by sohamb117; not affiliated with nativeMons. [PKHeX](https://github.com/kwsch/PKHeX) by Kaphotics and contributors. Pokédex entries from [PokéAPI](https://pokeapi.co). [OpenHome](https://github.com/andrewbenington/OpenHome) by Andrew Benington. Sprites served by [hailsDotGO](https://pogo.hails.app). Launcher design from the Skate 3 Rust Engine's [hails-additions](https://github.com/Hailey-Ross/rusty-trucks/tree/hails-additions) branch.

Not affiliated with or endorsed by Nintendo, Creatures Inc., GAME FREAK inc. or The Pokémon Company.
