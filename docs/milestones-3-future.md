# KaneegaBell milestones, part 3: ideas not discussed yet

Everything agreed so far (through u, customization) and the ideas already parked are in
[part 2](milestones-2-releases.md). This file collects ideas **nobody has discussed or approved
yet**: proposals only. Each one needs the owner's OK before it's planned, and new milestones go
**after** the locked order unless the owner decides otherwise.

## More platforms
Your PC already has the emulators for some of these, and the library has folders for others.
- **PS Vita** with Vita3K (installed in `E:\Emulators\Vita3K`). It likely needs its own scan rules,
  because Vita games are installed into the emulator rather than kept as single files.
- **PS4** with shadPS4 (your library has a PS4 folder). It's early days for that emulator: check
  which of your games actually run first.
- **Wii U** with Cemu, and **Dreamcast** (RetroArch's Flycast core or standalone Flycast), if you own
  games for them. KB already recognizes their folder names.
- Older systems RetroArch already covers (NES, Game Boy / Color, Genesis / Mega Drive, PC Engine…),
  if you want them in the library.

## Standalone emulators as alternatives
Since 0.3.1 a platform can have several emulators. You have standalone emulators installed that
could be offered ready-made, the way the Switch emulators are:
- PSP: **PPSSPP** · DS: **melonDS**, **DeSmuMe** · GBA: **mGBA** · N64: **Project64**, **M64Py** ·
  3DS: **Citra** · several systems: **ares**.
- Each would get checked launch options and a "Find emulators" pattern, and would join its platform
  once its exe is set.

## Controls
- **Button remapping** for KB's own controls (the brief's customization section lists it, but it
  isn't in u yet).
- **Per-game launch options:** an extra command-line flag for one game only (e.g. a different config
  file), within the "launch paths and templates only" rule.

## Speed (from the brief, not scheduled)
- **SQLite FTS5 search** across titles, developers and collections, so search stays instant as the
  library grows.
- **Watching game folders** for new or removed files (off by default), so rescans happen without
  pressing the button and without polling.
- **Startup snapshot:** remember the first screen (tab counts, first rows), so the grid appears
  before the full library has loaded.
- **Covers decoded off the main thread** and the next rows' covers loaded ahead of time.

## Library extras
- **Show hidden/missing games** as an option (the brief's customization section), greyed out, with
  "Locate…" to point KB at a moved file.
- **Card art choice** (cover / icon / background), once icons exist (after m).
- **Double-click a `.kbtheme` file** to import it, once themes exist (after o).

Before: [part 2, from 0.2.0 to the end of the plan](milestones-2-releases.md).
