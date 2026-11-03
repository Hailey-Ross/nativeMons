# mon-bridge

`tools/mon-bridge`: lets the launcher read cartridge saves and move Pokemon between them, using PKHeX.Core (https://github.com/kwsch/PKHeX, GPL-3.0-or-later).

## Commands

| Command | Output |
| --- | --- |
| `list <save>` | One tab-separated row per Pokemon in the boxes: box, slot, species, nickname, level, trainer, dex number, shiny/normal. |
| `info <save>` | Save type, game, generation, trainer. |
| `plan <from> <box> <slot> <to>` | What stands in the way: `forced`, `conflict`/`ok <n> <move>`, `item <name>`. |
| `caught <save>` | Dex numbers marked caught, comma separated. |
| `check <save>` | Corruption check: `ok` or `bad`, with why. |
| `compare <a> <b>` | Same trainer, Pokemon, Pokedex and items? |
| `repair <save> <new file>` | Rewrites a copy with correct checksums. |
| `families` | Evolution families: comma-separated dex numbers. |
| `moves <save>` | The moves that game has: id, name. |
| `move <from> <box> <slot> <to> <backup folder> [move:<1-4>:<id, 0 deletes>]... [dropitem] [at:<box>:<slot>]` | Moves one Pokemon, into that empty slot if given. |
| `layout <save>` | Boxes, slots per box, then a row per box: index, name. |
| `relocate <save> <box> <slot> <to box> <to slot> <backup folder>` | Within one save: moves it, or swaps it with what's there. |

Exit codes: 0 done, 1 usage or unreadable save, 2 the move isn't possible (saves untouched).

## Moves

- The destination is written first, so if anything fails after that, the Pokemon is duplicated, never lost.
- PKHeX has no route between some eras, and its converter never returns if asked, so those moves are copied field by field. The OpenHome converter (`tools/openhome-bridge`) keeps more when it's there. A hand copy loses what the older game can't hold, so the Pokemon's full data is kept for the trip back.

## FireRed saves

PKHeX tells Gen 3 games apart by one value in the save's header, and calls anything but 0 or 1 Emerald. Some real FireRed saves hold FFFFFFFF there. The party count settles it: it sits at a different place in each layout, and a real Emerald save always has 0 to 6 at Emerald's place. Stricter readers, Gen1Recomp's among them, only look at the header value, so a repaired copy gets the value FireRed normally has.

## Game options

`options <save> text=slow|mid|fast scene=on|off style=shift|set sound=mono|stereo button=normal|alt|lequalsa frame=1..20`

Settings a generation doesn't have are skipped. PKHeX has the locations for Gen 1 and 2. For Gen 3 and 4 they come from the decompilations, and a value that can't be real means the location is wrong, so nothing is written. Only a handful of bytes may change. It prints `ok` or `same`, or exits 2 with no change.

| Generation | Where | Layout |
| --- | --- | --- |
| 1 | 0x2601, in the block whose one-byte checksum (0x3523) is the inverted sum from the trainer name (0x2598) | text 3 bits, sound 2 (stereo = 2), style (set = 1), scene (off = 1); bit 3 kept |
| 2 | 0x2000, ahead of the area its checksums cover | same bits as Gen 1 |
| 3 | 0x13 button mode; 0x14 the rest | text 3 bits, frame 5, sound 1 (stereo = 1), style 1 (set = 1), scene 1 (off = 1) |
| 4 | directly before the trainer block | text 4 bits, sound 2 (stereo = 0), style 1, scene 1, button 2, frame 5 |
