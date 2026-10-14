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
- [ ] k. Resolution/DPI/multi-monitor pass + perf regression gate + WebView2 suspend test
- [ ] s. Keyless game info and better covers (section 13): LaunchBox Games Database → Wikidata
      series → Hasheous hash identification. **Do this right after k**: the sidebar (l),
      icons (m) and collections (n) all get better with its data. (Lettered s because it was added
      later, not because it comes last.)
- [ ] l. Game details sidebar + customization options
- [ ] m. Icons (game icons, platform/emulator icons) + app icon concepts → final icon
- [ ] n. Collections (manual, smart, automatic)
- [ ] o. Themes: format + built-ins → import/export → community gallery → optional AI generator
- [ ] p. Animations (Full/Reduced/Off)
- [ ] q. Multiple emulators per platform + per-game override (10a), then Switch (Eden default,
      Ryujinx, Yuzu), Xbox, Xbox 360 (one platform at a time)
- [ ] r. CHD/RVZ/CSO hashing for RetroAchievements

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
