# Stable check

`tests/stable-check/stable-check.ps1` decides whether a release package is fit to be the stable build. Run it before moving main.

It checks what we built around the engines, not the engines themselves: each game starts from our launcher and draws, saves stay sound, the package and updater work.

By default it's quick: one game per engine, the main screens and one update of each kind. Games and screens run `-Jobs` at a time (default 4), each launcher with its own state folder; one game never runs on two engines at once, since they share its save. Everything but play, window and sync opens no window and took 69 s (5 November). `-Full` tries every game on every engine and every screen. A release decision needs a full run of play, snapshots and updater.

The launcher is told not to take the keyboard, so the PC stays usable, though the games' own windows may take it briefly. Windows appear on screen during play and window: don't play games on the PC or move the mouse over them meanwhile.

## Runs

```
stable-check.ps1 -Package <zip> -Work <empty scratch folder> -RomFolder <folder> -Report <folder> [options]
stable-check.ps1 -Report <folder> -Retest            # what failed
stable-check.ps1 -Report <folder> -Groups saves,sync # chosen groups
stable-check.ps1 -Report <folder> -Skip play         # leave some out
stable-check.ps1 -Report <folder> -List              # where things are
```

Settings are remembered in the report folder, so later runs only need `-Report`. Give a setting again to change it, or as "" to clear it. Each group's latest result is kept, and the verdict always covers every group: a group that hasn't run counts against it. Exit code 0 means every group ran with no failures, 1 otherwise.

## Groups

| Group | Checks |
| --- | --- |
| repo | what's published: no games or attribution, pins, committed and pushed, dev build green |
| package | the zip: contents, licences, credit, nothing private |
| scan | finding ROMs, several dumps of a game |
| snapshots | every screen draws |
| saves | checks, several saves, export, moving a Pokemon, all on copies |
| play | every game on every engine shows a picture, and the launcher comes back (long) |
| sync | shared saves between engines, including damaged ones |
| window | real key presses through the real window, checked by screen name |
| updater | updates and Stable/Dev switching, from local releases (long) |

## Options

| Option | Meaning |
| --- | --- |
| `-Package` | the release zip to test (normally from the bleeding-edge release) |
| `-Work` | scratch folder, deleted at the end unless `-Keep` |
| `-RomFolder` | your ROMs. Only read, and listed before and after to prove it. |
| `-SaveFolder` | a folder of Fulldex .sav files. They're copied in; the originals are only read. |
| `-ArtFolder` | a `state\game-art` folder to copy in, so the run doesn't spend time capturing art |
| `-Report` | where results and screenshots are kept (they stay after `-Work` is deleted) |
| `-Jobs` | how many games or screens at once (default 4) |
| `-LauncherOverride` | a locally built launcher to test in place of the package's. The result then doesn't speak for the package: run again without it before releasing. |
