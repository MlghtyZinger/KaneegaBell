# KaneegaBell milestones, part 2: the releases (0.2.0 to now)

From the first published version to the current one. One milestone per release (the owner's rule
since 0.2.7). Full per-change lists are in the README's **Patch notes**.

| Version | Date | Milestone | What it brought |
|---|---|---|---|
| **0.2.0** | 2026-10-14 | i + j | Renamed to **KaneegaBell (KB)** with the library, play time and keys moved over automatically; per-user installer (no admin); **signed automatic updates** from GitHub Releases (Update now / Later / Skip). Code signing skipped for now. |
| **0.2.1** | 2026-10-14 | | Settings → Storage and reset: **Clear downloaded data** and **Reset KaneegaBell** (keeps a copy of the old library). |
| **0.2.2** | 2026-10-14 | k | Fits every screen from 720p to 4K and ultrawide; remembers monitor, size and position; TV Mode on a chosen display, text sizes, overscan margin; **everything in KB's own `data` folder** (keys encrypted there); almost no CPU while a game runs (WebView2 suspended); perf regression gate. |
| **0.2.3** | 2026-10-14 | | Private updates with an update access code. |
| **0.2.4** | 2026-10-14 | s, part 1 | **LaunchBox Games Database**, no key needed: covers, logos, backgrounds and game info; Fix match searches it too. KB keeps only what matches your games. |
| **0.2.5** | 2026-10-15 | s, part 2 | **Wikidata** series and franchises, no key needed. (Part 3, Hasheous, was dropped: LaunchBox already matched 103 of 105 games.) |
| **0.2.6** | 2026-10-15 | n, part 1 | **Collections** tab (your own collections, series as automatic ones, logos); series name, color and card in game details; **What's new**; slim platform sidebar; much less memory (~125 MB). |
| **0.2.7** | 2026-10-15 | l | **Game sidebar**: choosing a game opens a panel with background, logo, facts, play time, achievements, Play / Favorite / More…; full details on Triangle/Y. |
| **0.2.8** | 2026-10-15 | | Cover fixes (Refresh keeps your pick, no disc scans as covers), SteamGridDB searched by LaunchBox names, web cache capped at 32 MB. |
| **0.2.9** | 2026-10-16 | storage audit | Removed games take their badges and pictures with them; unused WebView2 add-ons removed (~20 MB); "Delete reset backup". |
| **0.3.0** | 2026-10-16 | r | **Compressed games keep their achievements**: CHD, CSO, ZSO, RVZ (WIA, GCZ, CISO) hash like the original dumps; converting a game keeps its play time and picks. |
| **0.3.1** | 2026-10-16 | q, part 1 | **Several emulators per platform** (RetroArch cores or your own), per-game "Launch with"; warnings for missing RetroArch cores or BIOS; a quick-closing emulator is explained instead of counted as played. |
| **0.3.2** | 2026-10-18 | q, part 2 | **Sega Saturn** (RetroArch Beetle Saturn, achievements, header detection, BIOS warning). Also: damaged Wii/CSO images can't crash KB, backgrounds/logos always refresh when changed, cue sheets without their discs skipped. |
| **0.3.3** | 2026-10-18 | q, part 2 | **Nintendo Switch**: Eden by default, plus Ryubing, Ryujinx, Citron, Sudachi and Yuzu; updates and DLC hidden; title IDs kept; covers for platforms without a libretro set; covers before Wikidata; clearer footer progress. |
| **0.3.4** | not released yet | | So far: one "Launch with" choice per game (details and sidebar); scene update files like `hr-bgw5av131072.nsp` hidden. The owner decides whether these ship alone or with Xbox. |

## Decided along the way
- **Release order locked** (2026-10-16): the owner asked to be reminded before any reorder.
- **Steam integration skipped:** games launch only through KB.
- **Separate code-split release (0.2.10) dropped:** Settings gets split in t, TV Mode in p.
- **Perf gate only when needed** (2026-10-18): for major updates or changes that touch startup, the
  grid or idle CPU; small fix releases skip it.
- **The installed copy is never touched**: changes go to the developer copy only.

## Also done between releases
- The 4 multi-disc games (FF7, Valkyrie Profile, Xenogears, Policenauts) were converted to CHD and
  checked (same achievement fingerprints); the originals are in `E:\CHD originals`.

Before: [part 1, building the app](milestones-1-foundation.md). Next: [part 3, the future](milestones-3-future.md).
