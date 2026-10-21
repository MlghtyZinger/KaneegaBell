# KaneegaBell (KB): Phase 2 brief

> Written 2026-10-13. Read this together with `CLAUDE.md` (the 2026-10-13 updated brief), which
> describes everything built so far (milestones 0 and a-h, performance numbers, controller map,
> platforms, metadata sources). This file adds the next phase. Where this file and `CLAUDE.md`
> disagree, this file wins, and any change to a goal or non-goal is marked **(changed)**.
> As before: propose a plan for each milestone and wait for the owner's OK before large changes.

## What stays the same
- Performance first. Every number in `CLAUDE.md` must still hold after each milestone
  (cold start < 1.5 s, 60 fps grid, idle CPU near 0, game-running mode minimal).
- Controller-first. Every new screen and option works with a controller alone.
- Zero setup. New features work without accounts. Optional keys go in the encrypted keys file
  (`data\keys.bin`, DPAPI), never in the DB, the repo or a shipped build.
- **(changed 2026-10-14) Everything in KB's folder.** All of KB's files (library, art, keys,
  WebView2 data, downloaded metadata) live in `data\` next to the exe, never in AppData,
  Documents or elsewhere (AppData only as a fallback when the folder isn't writable). New features
  must follow this. Downloads keep only what matches the user's own games.
- **Privacy:** never open the owner's files or other apps' data outside the project (e.g. a
  LaunchBox install, the registry) without asking first.
- Never download ROMs, BIOS, firmware, keys or any game content.

---

## 1. Name and identity: **KaneegaBell** (short: **KB**)
- Rename the product everywhere it's visible: window title, Tauri `productName`, exe name,
  Windows version info (`FileDescription`, `ProductName`, `CompanyName`), installer, README.
  The exe's version info is what fixes Task Manager showing "WebView2 Manager".
- **Data migration (do this carefully).** The Tauri `identifier` decides the app-data folder,
  so changing it would orphan the existing library, play time and favorites. Either keep the
  current identifier, or migrate the old data folder on first start (copy, verify, then
  switch; never delete the old one automatically). Also migrate the `EmuFrontend:*` Credential
  Manager entries to `KaneegaBell:*`, reading the old names as a fallback.
- **The exe path changes.** Update "How to work" (below) with the new path once it's decided.

### App icon
- The owner shared a screenshot of a World of Warcraft Tauren as the mood reference. That
  character is Blizzard's, so the icon must be an **original design** that fits the mood: bold,
  dark, a horned bull motif. It must not be a trace, recolor or redraw of that character or any
  other existing one. Don't commit the reference screenshot to the repo.
- Deliver as a vector master (SVG) plus a multi-size `.ico` (16, 24, 32, 48, 64, 128, 256)
  and the PNG sizes Tauri needs. It must read clearly at 16 px, so it needs a simple silhouette;
  a "KB" monogram variant is an option for the smallest sizes.
- Present 2-3 original concepts to the owner before finalizing.

## 2. Distribution and updates (do this early)
Goal: the owner's friend installs KB once, then gets updates like any normal app.

- **Installer:** Tauri's NSIS bundle (per-user install, no admin needed). It includes the
  WebView2 bootstrapper for the rare PC without WebView2.
- **Auto-update:** Tauri's updater plugin.
  - Updates are signed with an updater key pair. The public key ships in the app, and the
    private key stays with the owner only, never in the repo. **Back it up:** if it's lost,
    existing installs can never update again and everyone has to reinstall manually.
  - Releases are hosted on GitHub Releases with a `latest.json` manifest. The release assets must
    be publicly reachable, so either the repo is public or there's a separate public
    "releases" repo.
  - The app checks once in the background at startup (no polling loop). It shows the version
    and changelog and lets the user choose "Update now / Later / Skip this version". It never
    updates while a game is running; it installs on exit or on the user's OK.
  - A setting turns update checks off.
- **Code signing** (separate from updater signing): without it, Windows SmartScreen warns on
  first install. Options are an Authenticode certificate (paid) or Azure Trusted Signing (check
  eligibility for the owner's country). Explain the costs to the owner first. It's not required
  for updates to work.
- **Order matters:** the updater must be in the *first* build the friend installs. A build
  without it can't update itself, so everyone on it has to reinstall once.
- Release flow: bump version, build, sign, upload, publish manifest. Script it, and document
  it in the README. Publishing a release is outward-facing, so confirm with the owner each time.

## 3. Resolutions, scaling and multi-monitor
Check what already works before changing anything, then fill the gaps.
- Desktop and TV Mode work from 1280x720 up to 3840x2160, including ultrawide (21:9, 32:9)
  and Windows display scaling from 100% to 300%.
- Layout uses relative units and `clamp()`. Card sizes, text and spacing scale with the window
  and DPI, and nothing clips or overflows.
- TV Mode: an optional **overscan safe-area** inset for TVs that crop edges, plus text-size presets.
- Multi-monitor: the app remembers the window's monitor, size and position. TV Mode can target a
  chosen display (e.g., the TV). The app copes with displays being plugged in or out
  and with resolution or DPI changes while running.
- Add a resolution/DPI test matrix to the perf tooling (screenshots at each size).

## 4. Game details sidebar (desktop)
- An optional panel beside the grid that shows the **focused** game: cover or hero, title,
  platform, year, developer, genres, play time, last played, achievement progress, and the
  Play / Favorite actions.
- It follows focus with no extra input. Updates are debounced (about 100 ms) so fast scrolling
  stays smooth, and it only shows already-cached data (no network calls on focus).
- Turn it on or off in Settings and with a keyboard shortcut. For the controller, propose a
  binding: every button is already assigned (see `CLAUDE.md`), so suggest one (e.g., a
  long-press) and get the owner's OK. A still opens the full details screen.
- When the window is too narrow, the sidebar collapses automatically. The grid's virtualization
  must account for the narrower grid width.

## 5. Icons
- **Game icons:** a small square icon per game for list view, the sidebar, Recent, and
  collections.
  Sources, in order: a manual pick, SteamGridDB icons (optional key, same rules as its other
  art: static, SFW), then an icon cropped from the cover automatically. Cache them as small
  WebP files like the thumbnails, with a picker in the controller UI as for covers/heroes/logos.
- **Platform and emulator icons in the UI** (tabs, sections, settings): use original glyphs or
  permissively licensed icon sets, with licenses recorded in the README. Don't bundle official
  console or emulator logos (trademarks), especially since the app will be shared.

## 6. Collections tab
A **Collections** tab next to Favorites/Recent, with three kinds of collection:
1. **Automatic** (from data the app already has):
   - Series/franchise from IGDB's collection and franchise fields (needs the optional IGDB key;
     store the IDs, add them to the metadata fetch).
   - Developer and publisher (already stored from IGDB).
   - Without IGDB, *suggest* series from shared title prefixes (e.g. "Jak", "Final Fantasy"),
     which the user confirms. Never create them silently.
2. **Smart collections:** saved rules (platform, genre, developer, year range, favorite,
   played/unplayed, play-time ranges). They update automatically.
3. **Manual:** the user creates, renames, orders and adds games, all with the controller. A game
   can be in any number of collections.
- Collections can have a cover: one generated from the members' covers, or an image the user
  picks.
- Users can hide any automatic collection. Stored in SQLite (new migration).

## 7. Customization options
All of these go in Settings and work with a controller. Defaults match today's behavior.
- View: grid / list (list shows icons), card art (cover / icon / hero), card size (already
  exists), sort (title, last played, play time, release year, date added), sidebar on/off.
- Library: hide or reorder platform tabs, choose the start tab, hide empty platforms, show or
  hide hidden/missing games.
- Look: theme (section 8), accent color, font scale, animation level (section 9), clock in
  TV Mode on/off.
- Controller: button remapping (the exit combo is already configurable), glyph style (auto /
  Xbox / PlayStation / generic).
- Settings export/import as a file. Keys are never included.

## 8. Themes **(changed: the old non-goal "no themes marketplace" is narrowed)**
The owner wants to share themes with other people and download theirs. There will still be
**no accounts, no hosted server and no store**. Sharing is file-based plus a simple community
index.

### Theme format (data only, safe by design)
- A theme is a `.kbtheme` zip containing `theme.json` and optional assets (images, fonts).
- `theme.json` has: name, author, version, minimum app version, and **design tokens**: colors,
  fonts, radii, spacing, card style, focus-ring style, background (color, gradient or bundled
  image), separate desktop/TV Mode values, and an optional preferred animation level.
- **No JavaScript and no free-form CSS.** The app maps tokens to CSS variables. That keeps themes
  from breaking layout, controller focus or performance, and from running code.
- Validation on import:
  - Schema check, size limits, image re-encoding, and font type checks.
  - Zip-slip/path protection.
  - No remote URLs: all assets come from inside the file, and the CSP blocks everything else.
  - Minimum contrast for text and the focus ring; the app rejects or auto-corrects themes that
    would make focus invisible on a TV.
- Built-in themes: the current dark theme plus 2-3 others, all written in this same format.

### Getting themes
1. **Import from file** (in-app file browser, also double-clicking a `.kbtheme` once the
   installer registers the extension).
2. **Community gallery (keyless):** a public GitHub repo with an `index.json` (name, author,
   preview image, download URL, SHA-256). The app reads the index, shows previews, downloads,
   checks the hash, then runs the same validation as a file import. People submit themes by pull
   request, and the owner reviews them. Creating the repo is outward-facing, so ask the owner
   first.
3. **Export/share:** any theme, including one edited in-app, can be exported as `.kbtheme`.
4. **Claude makes them:**
   - Without any key: document the format in `docs/themes.md` (with a JSON Schema) so the owner
     can ask Claude in chat for a theme and import the result.
   - Optional in-app generator: the user describes a theme ("deep red, gold accents, stone
     texture") and KB asks the Anthropic API using the user's **own optional API key**, stored
     in the encrypted keys file like the others, with setup instructions in-app. The model returns
     tokens only. They're validated like any import and shown as a live preview before saving.
     Summarize current API pricing and terms for the owner before building this.

## 9. Animations (desktop and TV Mode)
- Animation levels in Settings: **Full / Reduced / Off**. The default is Reduced on desktop and
  Full in TV Mode. "Off" is also used when Windows' "Show animations" setting is off.
- Ideas: focus scale/glow on cards, smooth tab and section transitions, details/sidebar slide-in,
  cover fade-in on load, TV shelf easing (already exists), collection open transitions.
- Rules:
  - Animate only `transform` and `opacity`; no layout-triggering properties, no animated
    blur/filters on large areas.
  - No infinite animations on desktop. Idle CPU must stay near 0 with animations on.
  - Everything stops when a game launches (as today).
  - Measure frame times before/after with the perf tools; every frame must stay under 16.7 ms
    at Full on the 5,000-game library.

## 10. New platforms: Xbox, Xbox 360, Switch
Same process as the original 12. Verify CLI flags, supported formats and folder conventions
against each emulator's **current official docs/source at implementation time**, and summarize
them for the owner first.
- **Xbox:** xemu (expected formats: `.xiso`/`.iso`). It needs the user's own BIOS, MCPX boot ROM
  and HDD image, which the app never downloads.
- **Xbox 360:** Xenia (check whether Canary or the main build is the current recommendation). It
  needs disc images, `.xex`, and installed XBLA/GOD content folders, which need their own scan
  rules (like the PS3 folders).
- **Switch: three emulators, user's choice.** The owner has **Eden**, **Ryujinx** and **Yuzu**
  installed. Support all three as selectable Switch emulators:
  - **Eden** is the default. It's the most actively developed (a Yuzu-lineage fork).
  - **Ryujinx** is the alternative. Check which build the owner's exe is (original Ryujinx, or
    a continuation fork such as Ryubing), and verify the CLI against that build.
  - **Yuzu** is supported for completeness. It's discontinued and won't get updates, so treat
    it as a legacy option. Eden descends from Yuzu, so their CLIs are probably similar, but
    verify each separately and don't assume they're identical.
  - Each has its own exe path setting, default launch template and fullscreen flag, verified
    against the owner's installed versions (the `--help` output, if the project's site/source is
    unclear). Settings → Switch shows a picker: Eden / Ryujinx / Yuzu.
  - Most games use the platform default; a per-game override (section 10a) handles games that
    run better in another one.
  - Formats: `.nsp`, `.xci` (possibly `.nca`/`.nro`). Read titles/IDs from the files' metadata
    where possible. Keys and firmware are the user's own, and the app never downloads them. Each
    emulator keeps its own keys/firmware folder, which KB doesn't manage (non-goal: no
    emulator configuration).
  - These projects move hosts after takedowns. The README should say to get emulators only from
    their official sources, because fake installers circulate. KB never downloads emulators
    itself.
- Metadata/art: IGDB and SteamGridDB already cover these. GameTDB has no data for them, and
  libretro-thumbnails coverage varies by system, so check it.
- RetroAchievements: the existing runtime check of RA's active-console list decides; if a
  console isn't listed, no achievements section is shown.
- Platform icons (section 5) for each new platform.

## 10a. Multiple emulators per platform + per-game override
This is general, not Switch-only, and is useful later for PS2, GameCube, etc.
- A platform can have **several emulators configured** (each with its own path, core and
  template). One is the platform default.
- **Per-game override:** on the details screen, the controller picker "Launch with: Default
  (Eden) / Ryujinx / Yuzu" sets the emulator for that game, and the choice sticks.
  The launch-command preview shows the chosen one.
- Play time and last played stay per game, whichever emulator ran it.
- Optional: hold a button on Play to choose the emulator for one launch only. Propose a binding
  and get the owner's OK.
- DB: new migration (platform emulator list; nullable per-game emulator override). Existing
  single-emulator setups migrate automatically as the platform default.
- Unit tests: template rendering per emulator, fallback to the platform default when an
  overridden emulator's path is missing (show a clear message instead of failing silently).

## 11. Optimization ideas
The targets are already beaten by a wide margin, so the main job is to **stop new features
from eroding them**. In priority order:
1. **Perf regression gate:** run `tools/perf/perf.ps1` against the 5,000-game library after every
   milestone and fail if any number regresses by more than 10%. Record results in the README.
2. **Suspend WebView2 while a game runs:** WebView2 can suspend a hidden webview's renderer.
   That should cut CPU to near zero and lower memory further than today's low memory target.
   Measure it, then keep it if it's better.
3. **Code-split TV Mode, themes UI and the settings screens** so desktop start only loads the
   grid. Keep cold start flat as features are added.
4. **SQLite FTS5** for search across titles, developers and collections, so search stays instant
   as the library grows.
5. **Filesystem watching** for library folders (Windows change notifications, debounced, off by
   default or limited to a few folders). Rescans happen without a button and the app does no
   polling.
6. **Image work off the main thread:** decode covers asynchronously and prefetch the covers just
   beyond the visible rows. Use smaller icon thumbnails for list view.
7. **Startup snapshot:** cache the first screen's data (tab counts, the first visible rows) so
   the grid appears before the full DB query finishes.
8. Keep the WebView2-replacement idea deferred (see `CLAUDE.md`); revisit only for low-RAM
   devices.

## 12. Still carried over from `CLAUDE.md`
- Compressed disc formats: add CHD/RVZ/CSO reading to the RetroAchievements hasher **before**
  the owner converts the library, and verify that the hashes match the originals. The
  chdman/maxcso downloads need the owner's OK.
- Backloggd sync only if an official API appears.

## 13. Keyless game info and better covers (added 2026-10-14)
The owner wants game info and covers that work for everyone **without anyone entering API keys**,
and a better keyless cover source than libretro-thumbnails/GameTDB (they miss games and have no
backgrounds/logos). Research done 2026-10-14; re-check terms and formats before building.

**Candidates that need no key:**
1. **LaunchBox Games Database** (best fit, do first). `gamesdb.launchbox-app.com/Metadata.zip`
   (~103 MB, rebuilt daily, no account) holds every game's title, alternate names, platform,
   release date, overview, developer, publisher, genres, max players, rating, and the file
   names of its images, served from `images.launchbox-app.com/<file name>`: front boxes, **clear
   logos and fanart backgrounds** (so TV Mode gets logos/heroes without a SteamGridDB key),
   screenshots, discs, by region. RomM uses it as a provider.
   - Terms: there is **no published API or license** for outside use. LaunchBox staff have said on
     their forum that other apps already use the file and that the image names can be used to build
     image URLs. Be polite: download the zip at most weekly (only when it changed, by
     `Last-Modified`), stream-parse it into a small table for KB's platforms only, never ship it,
     cache images like today, credit "LaunchBox Games Database" in the UI/README. The owner may want
     to ask LaunchBox on their forum before the friend's build relies on it.
   - Matching: platform + normalized title + alternate names (the same rules as libretro), region
     preference, manual "Fix match" picker.
2. **Wikidata** (CC0, keyless SPARQL at query.wikidata.org; needs a descriptive User-Agent, has a
   query-time budget). One query per platform returns every game's labels, release date,
   developers, publishers, genres, **series (P179)** and IDs (IGDB P5794, MobyGames, etc.). Good
   for series collections (section 6) and developer/publisher without IGDB. No covers (non-free
   images aren't on Commons). Cache per platform for a month.
3. **Hasheous** (hasheous.org, free, AGPL, run by the Gaseous project). Its hash lookup
   (`/api/v1/Lookup/ByHash/md5|sha1|crc/...`) needs no key and maps No-Intro/Redump/TOSEC dumps to
   IGDB / RetroAchievements IDs: exact identification instead of title matching. Its IGDB metadata
   proxy needs an app-level "Client API Key" (registered once by the app's developer, not by each
   user); shipping that is an owner decision (see "keys never shipped" in CLAUDE.md). Rate
   limits aren't published: be gentle, sequential, cached.
4. **libretro-database** (GitHub, CC-BY-SA-4.0): per-system DAT files keyed by checksum with
   developer, publisher, genre, release year, franchise and serials. Fully offline once
   downloaded; credit required. Mostly cartridge/older systems; thin for PS2/PS3/Wii.

**Ruled out:** ScreenScraper (account needed; developer credentials embedded in open-source apps are
a grey area), TheGamesDB, RAWG, GiantBomb (all need keys), MobyGames (paid), GameFAQs/
TheCoverProject (no API, scraping not allowed). IGDB, SteamGridDB and RetroAchievements stay as
optional per-user upgrades.

**Owner's requirement (2026-10-14): only what matches.** LaunchBox offers no partial download (one
compressed XML) and no per-game API, so KB streams `Metadata.zip` and filters it as it arrives:
only entries matching the user's games are kept (a few MB at most in `data\`), the rest is never
stored. Re-download only when LaunchBox's file changed and there are unmatched games, or on
"Update game info". Images only for the user's games, preferring the box of the dump's region.

**Parked (owner, 2026-10-21): test ScreenScraper as a main metadata provider** (it has backs, spines "box-2D-side", discs/carts and
manuals, found by ROM hash; the owner plans manuals later). Not a milestone yet and not in the locked release order: needs the
owner's go for where it goes. API read 2026-10-21 (webapi2.php, partly): needs developer credentials (devid, devpassword,
softname) requested from ScreenScraper's forum by the app's developer, plus a member login (ssid/sspassword; anonymous use is
closed, 401); threads and per-minute/daily quotas depend on the account's level (429 threads/rate, 430 daily quota, 431 too many
unrecognised ROMs), and the scraper must manage quotas itself; API v2 is beta; integrations must be fully free and distributed
(otherwise prior authorisation). Never embed another app's credentials. Test plan: a throwaway probe script on the dev library's
games (hash lookups, which media each has, match rate, speed, vs LaunchBox), run with the owner's own login, before any KB code.

**Plan shape:** LaunchBox DB as the new primary keyless source for covers, logos, backgrounds and
info (libretro/GameTDB stay as fallbacks, IGDB/SGDB picks win when set up), then Wikidata for series,
then Hasheous hash identification. Settings → Game info shows each source, what it adds and its
credit; each can be turned off. All downloads stay in the background worker, cached, never on focus.

## Explicit non-goals (updated)
- No ROM, BIOS, firmware, key or game-content downloading of any kind (unchanged)
- **(changed)** No themes *store*, hosted server or accounts. File-based themes and a
  GitHub-hosted community index are allowed (section 8).
- No video snaps, music, plugin/scripting system or cloud sync (unchanged; themes are data only)
- No built-in emulator configuration beyond launch paths and command templates (unchanged)
- No account system (unchanged)

## Suggested milestone order
Each milestone is runnable and tested, and passes the perf gate.
- [x] i. Rename to KaneegaBell + data/credential migration + version info
- [x] j. Installer + auto-updater (+ code-signing decision: skipped for now); first release v0.2.0
- [x] 0.2.1: Settings → General → "Storage and reset": Clear downloaded data (keeps the library and
      the user's own picks, then refetches) and Reset KaneegaBell (back to first start, keeps
      `library.before-reset.db`, optionally removes saved keys)
- [x] k. Resolution/DPI/multi-monitor pass + perf regression gate + WebView2 suspend test (kept:
      game-running CPU 0.77 -> 0.31% of a core); plus everything moved into KB's own folder
- [x] s (part 1). LaunchBox Games Database: keyless covers, logos, backgrounds and info, LaunchBox
      first (owner's choice), storage 1b (name list for enabled platforms + owned games' details),
      filter-while-streaming; 91% of 1,776 real dump names matched.
- [x] s (part 2). Wikidata series/franchises (P179/P8345): per-platform queries, only matched entries
      kept, monthly recheck for unmatched; User-Agent contact = the owner's GitHub profile (their choice).
- [-] s (part 3). Hasheous hash identification: **dropped** (owner, 2026-10-15). LaunchBox matched 103
      of 105 dev-library games; the two misses are fan hacks, which Hasheous's DATs don't list either,
      and compressed files (CHD/RVZ/CSO) have other checksums than the dumps it knows. Compression is
      handled by milestone r instead. IGDB and SteamGridDB stay as optional upgrades (owner's choice).
- [x] l. Game details sidebar, released in 0.2.7 (customization moved to u, owner's choice). **Controls decided (owner, 2026-10-15):**
      choosing a game (pad ✕/A, Enter, double-click) opens the sidebar on the right with focus inside
      and Play ready (A again plays), B/Esc closes it; it has Play, Favorite and More…. △/Y, F and
      right-click open the full details screen. No favorite shortcut any more (favorite lives in the
      sidebar and details). Options/Start stays Settings; Esc opens Settings when there's nothing to
      close (back first). TV Mode unchanged. After this milestone: answer the owner's question about
      a Steam integration "like RetroArch does it".
- **Release order (owner, 2026-10-16), one milestone per release. LOCKED by the owner ("remind me to
      not change them later on"): if they ask to reorder, remind them first and park the idea as a note
      under its milestone; change only if they confirm.** ~~0.2.9 storage audit fixes~~ →
      ~~r (0.3.0, compressed-disc hashing)~~ → q part 1 (0.3.1, several emulators per platform) → q part 2
      (0.3.2 Saturn, 0.3.3 Switch, 0.3.4 Xbox, 0.3.5 Xbox 360 optional) → t (Settings
      touch-up, includes code-splitting Settings) → m (icons) → Steam-style list view → n rest, o, p,
      u. The separate code-split release (0.2.10) was dropped: it saves only ~10-30 ms today, so
      Settings is split in t and TV Mode in p.
- [ ] m. Icons (game icons, platform/emulator icons) + app icon concepts → final icon. **Split (owner, 2026-10-18):** part 1 = 0.3.6 (app icon + platform/emulator
      icons, bundled with the sidebar background fixes), part 2 = 0.3.7 (game icons). App icon direction (owner): a black
      bull-man with a gold nose ring, white-yellowish horns, his left horn broken halfway; smooth, not blocky. The owner
      allows the WoW Tauren as inspiration ("not publishing this app ever"); still drawn by Claude, not traced.
      **Picked: concept L** (round 5, black bull emblem on the ember tile, cream outline); master saved as
      `src-tauri/icons/icon-master.svg`. Next: generate the .ico/PNGs from it (`tauri icon`, output to a temp
      folder, copy only the sizes tauri.conf.json lists), then the platform/emulator icons. Icons show on
      the sidebar title, Recent, collections, platform tabs; the grid stays covers.
- [x] Steam-style list view (owner's idea, 2026-10-16; its own release right after m): a layout
      with games as text rows with their icon on the left, the focused game's info on the right (a wider
      version of the sidebar). Up/down moves, the panel follows. **Decided 2026-10-19 (owner):** design A1;
      it **replaces "One list"** (V now switches Grid ↔ List; Grid keeps its platform sections). Panel: the
      background as a banner, the whole cover over its lower edge, then a box each for Play, Achievements
      (the latest 5 unlocked, then the most valuable locked; click opens the details screen for now), Collections
      and series (if the game has any), Links and About. **Built 2026-10-19 (0.3.8):** `VirtualGrid` `rowHeight`
      (one column of fixed rows, platform headers kept), `GameListRow`, `GameSidebar` `docked`/`active` (A/Enter/→
      focus the panel, B/Esc back to the list; the panel hides below 900 px until opened), setting `listView`,
      `Glance` carries the 5 shown achievements and the RA game id.
- [ ] n. Collections (manual, smart, automatic). Part 1 done early (owner's request 2026-10-15):
      Collections tab, the user's own collections, series as automatic collections (2+ games,
      hideable), logos for collections and series (a game's logo, a cover, or the user's image).
      Left: smart collections (rules), developer/publisher collections, a TV Mode overview.
      **Owner's concern (2026-10-16):** adding lots of automatic collections would overcrowd the tab.
      Proposed instead: nothing new appears unless the user makes it; smart collections become
      saved filters (the genre filter grows into platform/genre/developer/year/played/play time,
      and a combination can be saved); developer/publisher become filter fields, not automatic
      collections. **Approved by the owner (2026-10-16); build it in n.**
      **Also for n (owner, 2026-10-18):** edit members from the collection page ("Add games…" picker with ticks,
      "Remove from collection" on Y/right-click); series get the same, saved as the user's corrections on top of
      Wikidata (a refresh never undoes them); "Search logos…" beyond the members' own (e.g. Armored Core offers only
      V and Verdict Day today): SteamGridDB search with the user's key (LaunchBox keeps only owned games' picture
      lists); own image file stays.
      **Member editing built 2026-10-19 (0.3.9):** collection editor has a searchable tick picker; Y/right-click removes
      a game from an open manual collection or series; series membership corrections persist across Wikidata refreshes.

      **Sorting and reordering built 2026-10-20 (owner's request, after 0.3.10's parts; its own release, not yet versioned):**
      the Collections tab has a sort picker (Select/G): A-Z (default), My order, newest/oldest release (of the latest/first game),
      recently played, most played, most games, genre (the genre most of its games share, no headings). Missing values sort last.
      "My order" lets you drag your own collections (hold a tile 250 ms, drag onto another) or pick one up (V / R3), move it with
      the D-pad and put it down with A. Series stay after yours. Still to decide: a binding better than R3, and genre headings.
- [ ] o. Themes: format + built-ins → import/export → community gallery → optional AI generator
      **Owner (2026-10-18): model the app's look around the icon** (concept L: black, ember red-orange, cream, gold)
      when themes come; likely the default or a built-in theme.
      **Plan decided (owner, 2026-10-20):** 0.3.11 = theme format + built-in styles the user switches between any time;
      0.3.12 = users make their own themes **in app** (token editor, live preview, save/export). Gallery download parked.
      **Styles (claude.ai artifact "KB Built-in Styles"):** Ember (**default**, cream focus), Midnight (today's look),
      Daylight, High contrast, Moss, Storm. Each style has its own **focus look** (Ember brackets, Midnight glow, Daylight
      gap ring, High contrast spotlight, Moss lift, Storm bar) and its own **bull icon colors** (window/taskbar icon
      follows the style; the exe's icon stays Ember). The per-style focus color choices in the artifact are offered as
      options in 0.3.12's customization.
      **Scrapped (owner, 2026-10-20): no in-app theme editor, no theme customization (colors, borders), no .kbtheme
      import/export for now.** The six built-in styles stay as they are; the editor code is in git history (8847eaa).
- [ ] p. **Split (owner, 2026-10-20): animations first as 0.3.12 (with the waiting panel fixes), the TV Mode touch-up
      after it as its own release (0.3.13).** Animations decided (owner): two pickers, Desktop and TV Mode, both
      **default Full**; **(changed)** Windows' "Show animations" is ignored (KB only reads it, never changes PC settings;
      users pick Off in KB instead), which overrides section 9's default and Windows rule. First an interactive mockup
      (artifact) to try Full / Reduced / Off on sample cards and panels, then build.
      **Mockup approved (owner, 2026-10-20, claude.ai artifact "KB Animation Levels"): "they look good, save them for
      later".** Owner then asked for a start-up animation and a game-launch animation like SwanStationPS5's (darkxex, GPL;
      3D jewel case opens, the disc spins on its tray while the game loads). Plan: original CSS 3D (transform/opacity
      only, no copied assets), per media type (CD jewel case, DVD/Blu-ray keep case, cartridge), runs only while the
      emulator starts; mockup first.
      **Decided (owner, 2026-10-20, artifact "KB Launch Animations"):** the launch animation plays on the desktop and in
      TV Mode, each with its own on/off option. LaunchBox disc pictures on the disc when they exist, else the cover.
      The TV shelf becomes 3D cases now (Claude's call, owner delegated; flat covers stay as an option), not in 0.3.13.
      Start-up intros: several, randomizable like the Steam Deck's boot videos: Random / one fixed / Off, desktop and
      TV separately. Six drawn so far: Ember, Horns, Forge, Disc, Power on, Your games (owner to pick which stay).
      **(changed) Sounds allowed (owner, 2026-10-20): sound effects and short tunes** (overrides the "no music" non-goal for
      UI sounds only; still no background music library or video snaps). Synthesized live with Web Audio (original, no
      files): intro tunes, case click/disc spin/cartridge clunk, soft move/select ticks. On/off + volume, desktop and TV
      separately, silent while a game runs.
      **Idea for the TV Mode touch-up (owner, 2026-10-20; plan proposed, waiting for the owner's OK): console-style TV
      layouts**, one per platform (Wii channels, PS1 memory-card/CD screen, PS2 browser, PS3 cross-bar, Switch home row,
      GameCube cube, DS/3DS dual screen, Xbox/360, retro CRT for cartridge systems). Homage only: original drawings,
      **Owner (2026-10-20): wants them ~99% like the real consoles ("never licensed or monetized"); approved the order:
      console layouts inside p across several releases (0.3.13 = layout system + the first ones with the touch-up), then
      u, then v.** Claude redraws everything by hand to look near-identical (layout, colours, motion, sound character);
      no extracted console files (logos, system sound/music recordings) are bundled. Mockups when the owner gets to it.
      **Intro lineup (owner, 2026-10-20, fix these before adding more):** Konami (Metal Gear), Rockstar (RDR2: the emblem
      is shot), Game Boy Advance, GameCube, PlayStation, PlayStation 2, Xbox (green emblem), Switch. Removed: Claude's own
      six, Assassin's Creed, THX, Game Boy, N64, Saturn. The emblem may be recoloured per intro. **Sound: the owner rejected
      every synthesized sound ("fake video games").** Proposed: real recordings the owner makes from their own emulators
      and BIOS (in KB: one sound file per intro in `data\`), CC0 sound packs for UI/case sounds only with the owner's OK.
      **Done instead (owner, 2026-10-21: "download any free sound library"):** every sound is mixed with ffmpeg from real
      CC0 recordings: VSCO 2 Community Edition (Versilian Studios; glockenspiel, marimba, piano, timpani, strings, horns,
      gong, cymbals) and The Free Firearm Sound Library (OpenGameArt; S&W 642 revolver, Marlin 336). Credit both in the
      README when this ships. The mixer script is `build_sounds.py` (now in `tools/sounds`).
      **Later decisions (owner, 2026-10-21):** lineup is now ten: Konami, Rockstar (one double-barrel shotgun blast with
      the owner's sound, a spread of pellet holes), GBA, GameCube, PlayStation (the old diamond version), PS2 (name on
      black), PS3 (name on the right), PSP (line → blue waves → white screen, grey text on the last riff), Xbox 360 (a
      big X behind the orb), Switch. Menu sounds: three profiles, **Tactile (default)**, Console, Retro console, each
      with move/select/back + disc start + cartridge start (Kenney CC0 clicks + generated tones, `make_profiles.py`).
      Cartridges, consoles and handhelds drawn from the real hardware as SVG (no logos; handhelds whole: GBA front,
      DS Lite and 3DS closed, Switch with Joy-Con). TV shelf cases can be inspected (turn ±80°, tilt, flip to the back).
      **The owner's own intro recordings** (`C:\Emulator files\audio files`, never committed): the owner calls them
      royalty-free; Claude's position, unchanged: KB ships only its CC0-based sounds, and a user's own files go in
      `data\intros\<intro>.ogg|mp3|wav|m4a|flac` (played from the start, with longer "custom" timings made for them).
      The owner's files were cut to the approved cues into the dev copy's `data\intros` (local only); the owner copies
      that folder into the installed copy themselves.
      **Built 2026-10-21 (0.3.12):** `src/state/fx.ts` (settings, defaults above, `html[data-anim]`), `src/fx/`
      (`audio.ts` Web Audio, `Intro.tsx`, `LaunchFx.tsx`, `Model.tsx` 3D cases, `hardware.ts` SVGs, `Stage.tsx`),
      `src/styles/fx.css` + `intro.css` (ported from the mockup), Settings → General: Animations, Start-up intro,
      Sounds; right stick X = new actions turnLeft/turnRight (keys `[` `]`), R3/V flips in TV Mode; `intro_sound` and
      `get_setting_values` commands; WebView2 `--autoplay-policy=no-user-gesture-required` (the intro's sound);
      LaunchBox keeps "Disc" and "Box - Back" images (stamp `kinds`, one extra download for older lists) stored as
      `art\d`, `art\b` and served as `thumb://…/d|b/<id>`; the launch animation plays before `launch_game`
      (KB minimizes at spawn), Back cancels it.
      **Owner's feedback on the first build (2026-10-21), fixed:** covers cropped (cases now take the cover's `artAspect`,
      same area; a launching cartridge keeps the platform shape), no spines (drawn ones rejected: use LaunchBox's "Box - Spine",
      `art\s`, tall ones fill the side, wide ones, a horizontal box's top spine, are turned; fallback = the cover's edge
      stretched + platform band + title), backs missing (backs download in the background; a miss is retried), non-English
      backs (English regions first for backs and spines; "Back cover…", "Spine…", "Disc…" pickers in the details screen,
      user picks marked `<id>.manual` and kept by "Clear downloaded data"). `IMAGE_KINDS_VERSION` 3 (one more download).
      Animations (Full/Reduced/Off) + **TV Mode touch-up** (owner, 2026-10-15: "TV mode needs a
      touch up"; deferred here to avoid getting sidetracked). **Owner's main point (corrected
      2026-10-15):** only the right side of any background is really seen; the middle and left are
      covered by the UI (info panel, tabs, shelf). Look for a layout/composition that shows the
      background properly. Boktai: the background is too zoomed in (cover-fit crop) or barely
      visible, not another game's art. Paper Mario's built-in logo was *not* the problem. Also seen:
      letterbox bars baked into some backgrounds (Fire Emblem Echoes), and dark logos hard to read on
      dark art (Echoes). Idea tried and shelved: an opaque base in the cover's color
      (`cover_accent`) under each background, the art masked to fade out toward the text, tabs and
      shelf, and a thin light outline on logos. Also: a Collections screen in TV Mode (see n).
      Boktai is one example of a general cropping/covering problem (owner, 2026-10-16): present 2-3
      layout mockups first, like the sidebar, before building. Code-split TV Mode here.
      Owner wants this looked at next session ("look into this tomorrow").
- [ ] q. Multiple emulators per platform + per-game override (10a), then Switch (Eden default,
      Ryujinx, Yuzu), Xbox, Xbox 360 (one platform at a time). **Part 1 built 2026-10-16 (0.3.1, owner's
      OK; owner named no extra emulators yet, so recommended defaults):** migration 015
      (`platform_emulators`, `platforms.emulator_choice`, `games.emulator_choice`, `emulators.custom`);
      built-in emulator always usable, RetroArch addable to standalone platforms (suggested cores
      swanstation/pcsx2/dolphin/citra), the user's own emulators ("Add emulator…") addable anywhere;
      `launcher::choose`: once > game pick > platform default; a game pick that can't run falls back
      with a notice (Now Playing), a once-pick errors. "Play with…" is a picker on the details screen and
      in the sidebar (no long-press: every button is taken). Achievements labels from RA's supported-
      emulator list (docs checked 2026-10: RetroArch, DuckStation, PCSX2, Dolphin, PPSSPP, melonDS;
      not RPCS3/Azahar). Part 2 (0.3.2+): new platforms + the owner's extra emulators.
      **Part 2 research (2026-10-16; owner's list; emulators are in `E:\Emulators`, run each one's
      `--help` once with the owner's OK before building):** one platform per release:
      0.3.2 **Saturn** (**built 2026-10-16**: platform `saturn`, core `mednafen_saturn`, RA console 39,
      header sniff with product number, BIOS warning, DETECT_VERSION 3; also cue sheets whose images are
      missing are skipped with a warning (the owner's Policenauts folder has only the patch + cues; the
      owner's RetroArch has all 4 Saturn cores and both BIOS files). RetroArch: Beetle Saturn / Kronos / YabaSanshiro; BIOS `sega_101.bin`,
      `mpr-17933.bin` in RetroArch's system folder; .cue/.chd/.ccd/.m3u; RA yes: the Sega CD hash already
      accepts "SEGA SEGASATURN "; add the header to Auto-folder sniffing) → 0.3.3 **Switch** (**built
      2026-10-18**: Eden built-in `-f -g`; Ryujinx 1.1.1403 (original, not Ryubing) `--fullscreen`; Citron,
      Sudachi, Yuzu `-f -g`, addable to Switch only (`SWITCH_ALTERNATIVES`); GUI exes print no `--help`, so
      flags checked in their argument parsers; Yuzu is the owner's AppData copy and crashes on its own;
      add-ons hidden by title ID low 13 bits ≠ 0, [UPD]/[DLC] tags or a version ≥ v65536; libretro has no Switch set,
      so the cover pass no longer requires a libretro system; a Switch emulator joins Switch when its exe is set
      (migration 017 for ones set earlier); "Play with…" now saves the pick as the game's emulator, owner's request;
      Ryubing added (exe also Ryujinx.exe, detected by folder))
      **0.3.4 Xbox + Xbox 360 built together 2026-10-18 (owner's request):** xemu `-full-screen -dvd_path` (checked in
      the owner's build); Xenia Canary `--fullscreen` (not installed yet: unchecked, `shortcut:` comment); XDVDFS
      marker offset decides XGD1 0x18310000 / XGD2 0xFDA0000 / XGD3 0x2090000, XISO 0x10000 = folder decides
      (verified on the owner's 4 discs); `.xiso` suffix dropped from titles; 360 .xex/GOD/XBLA not scanned yet.
      **Parked idea (owner, 2026-10-18, research only, not scheduled): suggest the best emulator per game.**
      Data: Ryubing/Compatibility `compatibility.csv` (title_id, game_name, labels, status playable/ingame/menus/
      boots/nothing, last_updated; keyless on GitHub, ~270 KB); Yuzu's frozen `compatibility_list.json` (Flathub,
      0 Perfect … 5 Won't boot, 99 untested; mostly 99) as a weak hint for the Eden family; no Eden list; EmuReady has
      no public API/data terms (ask its maintainers). Shape: keep only owned games' rows, match by title ID then name,
      label options in "Play with…", suggest only when clearly better, never switch on its own.
      (planned: Eden default,
      eden-emu.dev; Ryujinx = Ryubing, ryujinx.app, `--fullscreen <game>`; Yuzu legacy, `-f -g <game>`;
      .nsp/.xci; titles from file names + `[0100…]` title IDs, never decrypt; hide update/DLC files; no RA)
      → 0.3.4 **Xbox** (xemu `-full-screen -dvd_path <iso>`; RetroArch xemu core is experimental, as an
      alternative; XISO only; user's MCPX/BIOS/HDD; tell Xbox .iso apart by its header; no RA) → 0.3.5
      **Xbox 360** (optional, last; Xenia Canary `--fullscreen <game>`; disc ISO/XEX, GOD + .data, XBLA;
      no RA; only if the owner has games without PC ports).
- [ ] r. CHD/RVZ/CSO hashing for RetroAchievements (needed **before** the owner compresses any games;
      until then compressed games launch fine but show no achievements). **Built 2026-10-16 (0.3.0,
      owner's OK):** `src-tauri/src/disc/` reads CHD (`chd` crate) and CSO v1/v2/ZSO (own reader, own
      LZ4) as 2048-byte sectors for the PS1/PS2/PSP/Sega CD hashes and scanner sniffing; RVZ/WIA/GCZ/
      CISO/TGC through `nod` (Wii re-encrypted, `rebuild_encryption`). Migration 014 re-hashes compressed
      files marked "no hash". Converted files keep their game row (same platform + title + variant,
      old file gone **from the same folder**, exactly one candidate; report kind "replaced"; review fix:
      a copy elsewhere while a drive is unplugged must not take the history). Review fix 2: sector 0
      is track 1's `INDEX 01` for cue/bin too (stored pregap skipped, as DuckStation/PCSX2/RetroArch
      address discs and as chdman stores `PGTYPE:V…`); verified with a pregap copy: cue = chd = RA hash.
      rcheevos' own cue reader can't read raw bins with a stored track-1 pregap at all. Test tools (owner's OK): chdman
      from MAME 0.289 (SHA-256 checked), maxcso 1.13.0, the owner's DolphinTool; scratchpad only.
- [ ] t. Settings touch-up (owner, 2026-10-15): make Settings compact and more readable without costing
      performance (performance stays the top priority: no heavier rendering, settings screens stay
      code-split and load only when opened). Ideas to propose: shorter texts with details behind a
      "More" toggle, grouped rows instead of long paragraphs, one status line per source.
      **Design chosen (owner, 2026-10-18), from interactive mockups (claude.ai artifact "KB Settings Layouts"):**
      layout **E, search first**: a search box at the top finds any setting across all sections (results tagged
      with their section); with it empty, section chips show one section's compact rows (name · one status line ·
      main button · on/off switch). Details **follow focus**: a strip under the list explains the selected row, no
      "More" control. Controller: D-pad moves, A presses, X switches on/off, LB/RB change section, up from the first
      row reaches the search box (on-screen keyboard), B clears the search / goes back. Rejected: A rows, B list and
      detail, C summary and cards, D grouped list, F tile home; chevron and "More ▸" for details.
      **Built 2026-10-18 (0.3.5, owner's go-ahead):** `INFO` map (title, section, explanation, keywords) feeds both the
      details strip (focus/hover on `[data-detail]`) and search (plus every emulator and platform by name; name and
      keyword matches ranked first); `Row` (name · status · controls, lists and forms under it), `OnOff` switch
      (styled checkbox); results jump to the setting and focus it. Settings code-split with `lazy()` (62 KB chunk).
      Perf gate passed and improved: start 460 ms, Task Manager 116 MB at start (−8%), after a game −16%.
      **Redesigned before release (owner, 2026-10-18), layout G from the artifact "KB Compact Settings":** each section
      is two columns of one-line tiles (name · status, then a fixed button slot and a fixed switch slot so everything
      lines up); lists and forms open in a panel (`Row` `open`/`sub` → `SetPanel`); the bottom strip is replaced by a
      pop-up beside the setting after ~350 ms of hover or focus (no (i) button, no key for it); platforms are tiles
      with Edit… and an on/off switch; Check now opens What's new when up to date; Storage → Manage… adds
      "Clear web cache" (WebView2 `ClearBrowsingData` disk cache only). Every section fits 1600×900 without scrolling.
- [ ] Storage audit (owner, 2026-10-15; **right after 0.2.7 is uploaded**, as its own small update): list
      everything in both data folders with sizes, check every place KB writes files/cache rows for a
      limit or cleanup. Known items: delete `data\ra` badges **only for games that were removed**
      (owner's rule: keep them otherwise, they're small), offer to delete `library.before-reset.db`,
      disable WebView2 components KB doesn't use (Subresource Filter, speech model, ~18 MB), check old
      `api_cache` rows and partial LaunchBox downloads. Background: the web cache had grown to 312 MB
      (fixed in 0.2.7 with `--disk-cache-size`).
      **Audit done 2026-10-15 (read-only, on DB copies), release as 0.2.9.** Installed copy 94 MB:
      art 34, webview 31, ra 3.8, library.db 8.4 + wal 7.7, before-reset backup 8.8. Web cache already
      gone there (trimmed). Dev copy 244 MB, of which 162 MB is the old web cache: its DB is still at
      migration 12, so the 0.2.8 exe hasn't run on it yet; the one-time trim removes it on next start.
      Findings: all 639 RA badges belong to owned games (nothing to delete now); 4 art files (0.5 MB)
      of one removed game are left behind (art isn't deleted with its game row); `api_cache` is 4.5 MB,
      all fresh; no `.tmp`/`.part` leftovers anywhere; unused WebView2 components ~20 MB (Subresource
      Filter 11.3, component_crx_cache 4.5, speech 2.6, hyphenation 1.7). **Proposed fixes:** delete a
      game's art, `ra:progress`/`ra:gamehashes` rows and (when no owned game uses them) its RA badges
      when its row is removed, plus a one-time sweep of existing leftovers; disable the unused
      WebView2 components by switch; "Delete reset backup" button in Storage and reset; maybe clear
      the web cache with "Clear downloaded data". **Built 2026-10-16 (all five, owner's OK), for 0.2.9:**
      `prune_leftovers` after every removal + a once-at-startup sweep, `--disable-component-update`
      (verified: no add-on folders appear) + one-time removal of the old ones, "Delete reset backup",
      Clear downloaded data resets `webCacheTrimmed`. A copy of the installed library went 96 → 74 MB.
- [-] Steam integration: **skipped** (owner, 2026-10-15). Games launch only through KB, so no Steam
      shortcuts or Steam library import; putting KB on Steam ($100, Steamworks) and Discord status
      weren't wanted either. KB can be added to Steam by hand as one non-Steam game if ever needed.
- [ ] u. Customization (section 7, split out of l on 2026-10-15): sort (title, last played, play time,
      release year, date added), start tab, hide/reorder platforms, hide empty platforms, button glyphs
      (auto/Xbox/PlayStation/generic), text size, settings export/import (no keys). List view and card
      art wait for m (icons); accent color for o (themes).
      **Also for u (owner, 2026-10-18): the platform sidebar is getting crowded** (3 fixed rows + 16 platforms).
      Ideas to build here: group platforms by maker (Nintendo / PlayStation / Sega / Microsoft) as folding rows,
      open state remembered, A opens/closes with a controller (preferred); hide and reorder platforms (already
      in u); slimmer rows when the sidebar is open.
- [ ] v. Achievements window (owner, 2026-10-19; **placed last in the release order: it was added after the order
      was locked, so move it only if the owner says so**). A window of its own with all of a game's
      RetroAchievements information, opened by clicking the Achievements box in the list view's panel (and from
      the sidebar and the details screen, which today show the list inline, so it can move out of there):
      every achievement with badge, title, description, points, type (progression / win condition / missable),
      unlock date and hardcore mark, rarity; progress (earned / total, points, hardcore count); filters
      (all / unlocked / locked / missable) and sort (unlock date, points, rarity, RA's order); jump to the
      next achievement worth the most points; a link to the game's RetroAchievements page; refresh; the
      reasons when a game isn't recognized (as in `Achievements.tsx`). Works with a controller alone (D-pad
      moves, B closes, LB/RB switch filters). Reads the cache like the details screen: no hashing, no requests
      except Refresh. Open question: the user's own latest unlocks across all games (a "recent achievements"
      view), which RA's API offers, would be a separate idea.

## How to work (carried over, with additions)
1. Propose a plan for each milestone and **wait for the owner's approval** before writing large
   amounts of code.
2. Before integrating any new service or tool (updater hosting, code signing, Anthropic API,
   new emulators), read its current docs and summarize auth, limits, costs and terms for the owner.
3. Unit tests for every new parser and rule: theme validation (including malicious zips),
   collection rules, the scan rules for the new platforms, and update-manifest handling. Ignored
   live tests run against **copies** of the real library DB only.
4. Keep the README current: setup, keys, adding a platform, making/sharing themes, the release
   process, performance results.
   **Patch notes** (README, last section; owner's request 2026-10-14): every user-visible change gets
   a line under the next version's `### X.Y.Z` heading as it's made, newest version first, **one
   change per line** (`- New:` / `Changed:` / `Improved:` / `Fixed:`), short and written for players.
   `release.ps1` uses those lines as the update window's changelog.
5. Commit at sensible points with clear messages.
6. How the owner runs the app:
   `C:\Emulator files\src-tauri\target\release\KaneegaBell.exe` (renamed in milestone i; it was
   `emu-frontend.exe`). This development copy has **its own separate library** in
   `src-tauri\target\release\data` (the owner's choice; `cargo clean` deletes it); the installed copy
   (`C:\Games\KaneegaBell`) keeps the real library in its `data\` folder. After changes, run
   `npm run tauri build -- --no-bundle` and tell the owner it's ready. Check for a running
   instance first and ask them to close it, because the build can't overwrite a running exe.
   Never drive or launch the app while they're using it.
7. Never download game content; confirm before destructive or outward-facing actions
   (publishing releases, creating repos, data migration).
8. **Adjustments agreed 2026-10-15:**
   - One milestone per release (storage audit = 0.2.9, m = 0.3.0, ...). Remind the owner when a
     request belongs to another milestone.
   - The perf gate's memory check uses the Task Manager figure (steady); private memory is still
     printed but can spike on a new exe's first launch, so it only warns.
   - Code-split TV Mode and Settings (load when opened) before m/o/t add more screens.
   - Release once, from a clean committed tree (`release.ps1` builds whatever HEAD is when it runs).
   - Don't run builds, perf runs or scans while the owner is playing; stop at once if asked.
   - **(2026-10-18) Perf gate only when needed:** major updates (new screens, icons, list view, themes,
     animations, TV Mode) or changes that touch startup, the grid, idle CPU or game-running mode. Small fix
     releases skip it (say why in one line). This overrides "after every milestone" in section 11.
   - Never change anything in the owner's installed copy (`C:\Games\KaneegaBell`); only the developer copy.
