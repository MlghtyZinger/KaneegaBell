# KaneegaBell milestones, part 2: from 0.2.0 to the end of the plan

Everything from the first published version through the last agreed milestone (u, customization):
what's released, what's planned in the locked order, and ideas already discussed and parked.
One milestone per release (the owner's rule since 0.2.7). Full per-change lists are in the README's
**Patch notes**.

## Released

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
| **0.3.4** | 2026-10-18 | q, part 2 | **Xbox** (xemu) and **Xbox 360** (Xenia Canary), told apart by disc layout; one "Launch with" choice per game; background progress in the top bar; fullscreen restored reliably after a game; scene Switch updates hidden. |
| **0.3.5** | 2026-10-18 | t | **Settings on one screen**: search box, two columns of one-line settings with buttons and switches lined up, forms in small panels, explanations pop up beside the setting; Check now shows what's new; Clear web cache; Settings loads only when opened. |

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

## Planned (the locked order)
The order is **locked** (owner, 2026-10-16): before changing it, the owner is reminded, the idea is
parked as a note under its milestone, and the order changes only if the owner confirms. Each
milestone is planned and approved before it's built.

1. ~~q, part 2: new platforms~~ done in 0.3.4 (Xbox and Xbox 360 together, owner's request). Left for
   later: Xbox 360 `.xex` folders, Games on Demand and XBLA content; checking Xenia's launch option
   once it's installed.
2. ~~t: Settings touch-up~~ done in 0.3.5 (search first, two columns of one-line settings, forms in panels,
   explanations pop up on hover or focus).
3. **m: Icons.** Small square game icons (manual pick, SteamGridDB, or cropped from the cover) for the
   sidebar title, Recent and collections (the grid stays covers); original or permissively licensed
   platform and emulator icons, no official logos; app icon: 2–3 original concepts (bold, dark, a
   horned bull), then the final icon.
4. **Steam-style list view** (its own release right after m): games as text rows with their icon, the
   focused game's info on the right; up/down moves and the panel follows.
5. **n, the rest: Collections.** Nothing new appears unless the user makes it. Smart collections become
   saved filters (platform / genre / developer / year / played / play time, combinations saved);
   developer and publisher become filter fields; a Collections screen in TV Mode. Also (owner,
   2026-10-18): add and remove games from the collection page itself, for series too (your corrections
   survive Wikidata refreshes), and "Search logos…" beyond the games you own (SteamGridDB).
6. **o: Themes.** A safe, data-only `.kbtheme` format (no code), built-in themes, import/export, a
   community gallery on GitHub (owner asked first), an optional generator using the user's own
   Anthropic API key.
7. **p: Animations and TV Mode touch-up.** Full / Reduced / Off (only `transform` and `opacity`, no idle
   CPU, every frame under 16.7 ms). TV Mode backgrounds are mostly hidden behind the UI (Boktai too
   zoomed in, letterboxed art, dark logos on dark art): 2–3 layout mockups first. TV Mode code-split.
8. **u: Customization.** Sort (title, last played, play time, release year, date added), start tab,
   hide/reorder platforms, hide empty platforms, button glyphs (auto / Xbox / PlayStation / generic),
   text size, settings export/import (never keys). Also (owner, 2026-10-18): the platform sidebar is
   getting crowded: group platforms by maker (Nintendo / PlayStation / Sega / Microsoft) as folding
   rows, plus slimmer rows.

## Discussed and parked (not scheduled)
- **Best emulator per game** (under q): label each emulator in "Launch with" with its compatibility,
  e.g. "Ryubing · Playable". Data: Ryubing's `compatibility.csv` (by title ID); Yuzu's frozen list as
  a weak hint for the Eden family; no Eden list exists; EmuReady has no public API.
- **Wikidata speed:** run each platform's two queries at the same time (Switch: ~21 s → ~16 s).
- **Backloggd sync:** only if Backloggd publishes an official API.
- **Code signing:** only if KB is shared widely (removes the SmartScreen warning; costs money).
- **Replacing WebView2 with a native UI:** deferred; only for low-memory devices (handheld PC, TV box).

Before: [part 1, building the app](milestones-1-foundation.md). Next: [part 3, ideas not discussed yet](milestones-3-future.md).
