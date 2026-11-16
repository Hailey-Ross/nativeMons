# PC games

[Back to the README](../README.md)

Pokémon Infinite Fusion and Infinite Fusion 2, and any other self-contained PC game you add.

## Pokémon Infinite Fusion and Infinite Fusion 2

[Pokémon Infinite Fusion](https://github.com/infinitefusion/infinitefusion-e18) and
[Infinite Fusion 2](https://github.com/infinitefusion/infinitefusion-hoenn-public) (Hoenn) are PC fan games (RPG
Maker XP on mkxp-z), so they need no ROM. Open one in **Games** and choose how to bring it in. Either way works, and
you can switch later:

- **Import it into the launcher**, from your installed folder, from the game's downloaded zip, or straight from its
  GitHub (that one needs [Git for Windows](https://git-scm.com)).
  - The launcher keeps the game in `games\infinitefusion` and its saves in `state\saves\infinitefusion`, both inside
    the launcher's folder. Saves you already have in `%APPDATA%\infinitefusion` are copied in, and the originals stay.
  - **Update** gets the newest release, the same way the game's own `INSTALL_OR_UPDATE` does.
  - **Uninstall** removes the game's files and keeps its saves.
- **Add an existing installation.** The launcher runs your own copy where it is and never changes its files, and the
  saves stay where the game keeps them, in `%APPDATA%\infinitefusion`. Update it with the game's own updater.
  **Remove from the launcher** only forgets it.

Infinite Fusion 2 works the same way, with its own folders: `games\infinitefusion2`, `state\saves\infinitefusion2` and
`%APPDATA%\infinitefusion-hoenn`.

Good to know:
- **Space:** about 5 GB for Infinite Fusion, about 3 GB for Infinite Fusion 2.
- **Backups:** the launcher backs up the saves before each session (the newest 30, under `state\save-backups`), and
  the game keeps backups of its own too.
- **Looks:** the Play tile and wallpaper come from each game's own title screen and region map.
- **My Collection:** both count toward it, each with a Pokédex of its own under **Romhacks and fan games**.
- **Not available for them:** Move Pokemon, quick start and the save tools.

## Other PC games

**Add a PC game**, at the end of **Games**, adds any self-contained game that has its own program, such as another fan
game. Nothing needs writing for it. Three ways in:
- **From its folder:** point to where the game lives. It runs there, and nothing is copied.
- **From a .zip:** the launcher unpacks it into `games\` and keeps that copy. The zip stays as it is.
- **Choose its program (.exe):** then run it where it is, or let the launcher copy the game's folder into `games\`.

The launcher finds the game's program itself, leaving out installers and uninstallers, and asks which one if there's
more than one. The game is named after its program (rename it on its page) and gets a color of its own.

These games go without the extras: no captured title screen or wallpaper, and no save handling, since the launcher
can't know where such a game keeps its saves. The launcher starts the game, shows it on Discord, and waits for it to
close. If the program only hands over to another one in its folder and quits, the session lasts until that one closes
too. **Remove from the launcher** forgets the game, and deletes the launcher's copy if it made one; your own folder
and saves are never touched.
