# KaneegaBell milestones, part 3: the future

The order below is **locked** (owner, 2026-10-16). Before changing it, KB's developer reminds the
owner, parks the idea as a note under its milestone, and changes the order only if the owner confirms.
Each milestone is planned and approved before it's built, and ships as its own release.

## The locked order

### 1. q, part 2 (the rest): new platforms
- **0.3.4 Xbox:** xemu (`-full-screen -dvd_path <iso>`), with RetroArch's experimental xemu core as
  an alternative; XISO images; tell Xbox `.iso` files apart by their header; the user's own
  MCPX/BIOS/HDD (never downloaded); no RetroAchievements. Needs the owner's OK to check xemu's options first.
- **0.3.5 Xbox 360 (optional):** Xenia Canary (`--fullscreen <game>`); disc ISO/XEX, GOD + .data, XBLA;
  no RetroAchievements; only if the owner has games without PC ports.
- Pending fixes (one "Launch with", scene update files) ship with 0.3.4 or just before it.

### 2. t: Settings touch-up
Make Settings compact and easier to read without costing performance: shorter texts with details
behind "More", grouped rows, one status line per source. Settings screens load only when opened
(code-split).

### 3. m: Icons
- Small square **game icons** (manual pick, SteamGridDB, or cropped from the cover) for the sidebar
  title, Recent and collections; the grid stays covers.
- **Platform and emulator icons:** original or permissively licensed, no official logos.
- **App icon:** 2–3 original concepts (bold, dark, a horned bull), then the final icon (SVG + .ico).

### 4. Steam-style list view
A third layout: games as text rows with their icon on the left, the focused game's info on the right
(a wider sidebar). Up/down moves and the panel follows.

### 5. n, the rest: Collections
Nothing new appears unless the user makes it. **Smart collections become saved filters:** the genre
filter grows into platform / genre / developer / year / played / play time, and a combination can be
saved. Developer and publisher become filter fields. A Collections screen in TV Mode.

### 6. o: Themes
A safe, data-only `.kbtheme` format (colors, fonts, shapes; no code), built-in themes, import/export,
a community gallery on GitHub (asks the owner before creating it), and an optional theme generator
using the user's own Anthropic API key.

### 7. p: Animations and TV Mode touch-up
- Animation levels Full / Reduced / Off (only `transform` and `opacity`, no idle CPU, every frame under 16.7 ms).
- **TV Mode layout:** backgrounds are mostly hidden behind the UI (Boktai too zoomed in, letterboxed
  art, dark logos on dark art): 2–3 layout mockups first. TV Mode gets code-split here.

### 8. u: Customization
Sort (title, last played, play time, release year, date added), start tab, hide/reorder platforms,
hide empty platforms, button glyphs (auto / Xbox / PlayStation / generic), text size, settings
export/import (never keys).

## Parked ideas (not scheduled)
- **Best emulator per game** (under q): label each emulator in the game's emulator choice with its
  compatibility, e.g. "Ryubing · Playable". Data: Ryubing's `compatibility.csv` (by title ID) and
  Yuzu's frozen list as a weak hint for the Eden family; no Eden list exists; EmuReady has no public API.
- **Wikidata speed:** run each platform's two queries at the same time (Switch: ~21 s → ~16 s).
- **Backloggd sync:** only if Backloggd publishes an official API.
- **Code signing:** only if KB is shared widely (removes the Windows SmartScreen warning; costs money).
- **Optimization ideas:** SQLite FTS5 search, watching game folders for changes, a startup snapshot
  of the first screen, decoding covers off the main thread.
- **Replacing WebView2 with a native UI:** deferred; only worth it for low-memory devices (handheld PC, TV box).

Before: [part 2, the releases](milestones-2-releases.md).
