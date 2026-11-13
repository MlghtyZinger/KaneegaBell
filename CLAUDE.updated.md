# Project: A fast, controller-first emulation frontend (lightweight LaunchBox alternative)

> Updated 2026-10-13. This replaces the original brief: it keeps the original goals, records
> what has been built, every decision that changed the original plan (and why), known
> limitations, and the owner's proposals for future work. Sections marked **(changed)** differ
> from the original brief.

## Goal
A desktop game library frontend for emulators, similar in purpose to LaunchBox/Big Box, with
performance as the top priority and a deliberately small feature set. I own all hardware and
games involved; this app only organizes and launches my existing dumps and emulators. It must
never download ROMs, BIOS files, or any game content.

Target OS: Windows 10/11 (keep paths and process handling cross-platform-friendly where cheap).

**Added goals (from the owner during development):**
- **Desktop first, TV Mode second.** The main experience is a windowed desktop app (mouse,
  keyboard and controller). A separate, switchable Big-Picture-style **TV Mode** adds the eye
  candy. Desktop mode stays lean (idle CPU near 0, no decorative animation).
- **Zero setup for anyone.** Other people should be able to launch the app and have scanning,
  the library and box art work without registering anywhere. Services that need accounts
  (IGDB, SteamGridDB, RetroAchievements) are optional per-user upgrades with in-app setup
  instructions; keys are never shipped in the app.
- **Grows with the collection.** Switch and other platforms (PS4, Xbox, Xbox 360, Saturn...)
  must stay easy to add; nothing may permanently exclude a platform in code.

## Tech stack **(changed)**
- **Tauri v2 + Rust** backend; the UI renders in **WebView2** (the system's Chromium engine,
  preinstalled on Windows 10/11), which keeps the app ~10-15 MB instead of bundling a browser.
  A custom engine or native Rust UI toolkit was discussed and rejected for now (a full UI
  rewrite to save ~250 MB of RAM, with all performance targets already met). Revisit only for
  low-RAM devices (handheld PC, TV box), by prototyping TV Mode first.
- **SolidJS + TypeScript + Vite** (fine-grained signals, no virtual DOM, tiny runtime).
- **SQLite via rusqlite** (bundled, WAL), migrations via `PRAGMA user_version` (001-008).
- **Controllers: SDL3, not gilrs.** gilrs' default Windows backend (Windows.Gaming.Input) only
  delivers input to the focused window, so the exit combo couldn't fire while an emulator had
  focus, and its XInput backend can't see native PlayStation pads. SDL3 (XInput + raw input +
  HIDAPI) runs with background events on, doesn't touch PS pads' report mode/LEDs, and
  de-duplicates a physical pad and its Steam Input/DS4Windows virtual copy. The owner plays
  with PlayStation controllers (native and via Steam/DS4Windows); hints use PS glyphs then.
- **Secrets:** Windows Credential Manager (`EmuFrontend:*` entries) via the `windows` crate;
  non-Windows fallback is a git-ignored `secrets.local.json`. Never in the DB or the repo.
- **Images:** `image` + `webp` crates; small WebP thumbnails for the grid, a larger WebP for
  details, heroes/logos for TV Mode; served by a custom `thumb://` protocol.
- **HTTP:** `ureq` (blocking, background threads only); URLs/keys never appear in errors/logs.
- **RetroAchievements hashing:** a Rust port of the rcheevos hashing rules (not C FFI as first
  planned), verified against real dumps.

## Performance requirements (acceptance criteria) - all met
Measured on a generated 5,000-game library, every game with a cover, release build, Ryzen 7
7700X / RTX 4070 Ti / 32 GB:

| Target | Result |
|---|---|
| Cold start to interactive < 1.5 s | ~530 ms median (512-667 ms) |
| Smooth 60 fps grid scrolling | 240 fps (every frame ~4.2 ms, none over 20 ms), first pass included |
| Idle CPU near 0% | 0.3-0.9% of one core (~0.05% of the CPU) |
| Modest RAM | ~280 MB private after start (app itself 13 MB; the rest is WebView2). Task Manager shows ~120 MB on the real library |
| Background scans | 5,000 games in ~0.3 s; rescan ~0.15 s |
| Minimal resources while a game runs | grid unmounted, window minimized, WebView2 memory target set to low: ~360 MB private, ~0.9% of one core |
| TV Mode | holding right through 5,000 games: every frame < 14 ms; idle 0.2% of one core |

Reproduce with the generator (`PERF_ROOT=<empty folder> cargo test --release --lib
generate_perf_library -- --ignored`) and `tools/perf/perf.ps1 -Data <PERF_ROOT>/data`.
`EMU_FRONTEND_DATA_DIR` runs the app on another data folder; such a profile never reads or
writes the saved API keys.

## Controller support - done
- Every screen works with a controller alone (Xbox, PlayStation, generic); mouse and keyboard too.
- A = select, B = back, LB/RB = platforms (or Settings sections), LT/RT = jump by letter,
  X = search (on-screen keyboard), Y = favorite, Start = Settings (TV Mode: menu),
  Select/Share/View = genre filter, right stick = card size, R3 = platform sections on/off,
  **L3 = TV Mode on/off** (keyboard F10).
- Exit-emulator combo configurable, default Select + Start held 1 s: polite WM_CLOSE first,
  then the whole Job Object tree is terminated after 4 s.
- Dropdowns are controller pickers; an in-app file/folder browser replaces the native dialog.

## Supported platforms and emulators - done (12 platforms)
Launch templates are tokenized first, then placeholders substituted (`{emulator}`, `{core}`,
`{rom}`, `{rom_dir}`, `{rom_name}`), no shell. Defaults (verified against the emulators'
docs/source); every platform's core, template and extensions can be overridden in Settings:

| Platform | Emulator | Default template / core |
|---|---|---|
| SNES | RetroArch | `-L "{core}" -f "{rom}"`, snes9x |
| Nintendo 64 | RetroArch | mupen64plus_next |
| Game Boy Advance | RetroArch | mgba |
| Nintendo DS | RetroArch | melondsds |
| Sega CD | RetroArch | genesis_plus_gx |
| PSP | RetroArch | ppsspp |
| GameCube / Wii | Dolphin | `-b -C Dolphin.Interface.ConfirmStop=False -C Dolphin.Display.Fullscreen=True -e "{rom}"` |
| PlayStation | DuckStation | `-batch -fullscreen -- "{rom}"` |
| PlayStation 2 | PCSX2 | `-batch -nogui -fullscreen -- "{rom}"` |
| PlayStation 3 | RPCS3 | `--no-gui --fullscreen "{rom}"` (EBOOT.BIN) |
| Nintendo 3DS | Azahar | `-f "{rom}"` |

Scanning: per-platform extension allowlists (saves/states/patches never listed); `.cue`/`.m3u`
referenced files hidden; multi-disc grouping by `(Disc N)`, with a generated `.m3u` in the app's
data folder (never next to ROMs) for emulators that support playlists; "Auto" folders detect
platform from headers (GameCube vs Wii, PS1 vs PS2 via SYSTEM.CNF, PSP, Sega CD) and skip
junk/cache folders; PS3 disc folders (`PS3_GAME`, also nested) and RPCS3 `dev_hdd0` installs
(`CATEGORY == HG` only) from PARAM.SFO; missing files hide entries instead of deleting play
time/favorites; each scan reports games added/removed/hidden/moved per platform.

Play time and last played are tracked (the only stats). The owner's real library:
`E:\Emulator Games` (~90 games across all 12 platforms), emulators in `E:\Emulators`.

## Metadata and artwork sources **(changed)**
All keys are entered in Settings → Game info and stored in Windows Credential Manager.

1. **libretro-thumbnails** (keyless, primary box art): matched by No-Intro/Redump name, then
   cleaned title with region preference, sequel numbers must agree.
2. **GameTDB** (keyless): covers by the game's own ID (PS3 title ID, GC/Wii disc ID, DS/3DS
   code); first choice for PS3.
3. **IGDB** (optional key, Twitch client credentials): title, description, release date,
   developer, publisher, genres (genre filter), cover fallback, and the IGDB slug (for
   Backloggd). 4 req/s, sequential; searches can't be batched (multiquery returns nothing for
   `search`). "Fix match" screen; manual picks are never replaced.
4. **MobyGames: dropped** (its API now costs $9.99/month, non-commercial only).
5. **SteamGridDB** (optional key, Bearer token): background pass matches games by title (no
   IGDB/console ID lookup exists) and stores the best-voted hero (wide background) and logo for
   TV Mode, plus covers for games no other source has. Static, SFW, non-humor, non-epilepsy
   images only. No published rate limit: sequential, 250 ms apart, back-off on 429. Pickers for
   cover / background / logo from the controller UI; choices stick.
6. **RetroAchievements** (optional username + Web API key): see below.
7. **Backloggd**: no public API (checked Oct 2026) and scraping is not allowed, so only a link.

Cache everything (API responses in `api_cache`, images on disk); the app works offline after
the first fetch. Not-found lookups retry weekly.

## RetroAchievements - done
- Games identified by RA's own hash (rcheevos rules ported to Rust in
  `src-tauri/src/achievements/hash.rs`): whole-ROM MD5 (GBA), header-stripped (SNES),
  byte-order-normalized (N64), header + ARM9 + ARM7 + icon (NDS), boot executable via ISO9660
  (PS1/PS2/PSP from `.cue`/`.bin`/`.iso`), first sector (Sega CD), apploader + DOL (GameCube,
  Wii `.iso` and `.wbfs` via its block table), PARAM.SFO + EBOOT (PS3 folders).
- Per-console hash lists cached a week; the active-console list decides support at runtime.
  3DS shows nothing (hashing needs console keys). Unsupported consoles show no section at all.
- Game details: progress bar, points, hardcore count, every achievement with badge, unlock
  date or rarity; offline from cache; refresh.
- Not recognized → explains why: if RA has a game by that title, lists the exact versions RA
  accepts (other regions, or versions needing an official "RA patch"); otherwise "no
  achievements found" (title-matched, so a game RA lists under a Japanese title can land there).
- Real-library findings: all mismatches were version differences (e.g. Jak 1-3 need RA's Debug
  Patch; Luigi's Mansion only EU). The owner's Wii `.wbfs` files lack the update partition that
  RA's Wii hash reads, so they can't match; a fresh full dump (ISO/RVZ) would.

## UI - done
- Desktop library: platform tabs (All, Favorites, Recent, platforms) with counts, virtualized
  grid; cards take their cover's shape; mixed views (All/Favorites/search) in per-platform
  sections; zoomable cards; search across platforms with an on-screen keyboard; genre filter;
  favorites; scan/art/info/artwork/achievement job progress in the footer.
- Game details: cover (change/refresh/remove, background/logo pickers), metadata, genres,
  description, play time, Play, Favorite, Fix match, achievements, launch command preview,
  and **Links under the cover** (Backloggd, IGDB, RetroAchievements; https-only allowlist).
- **TV Mode** (milestone g): fullscreen; focused game's SteamGridDB hero crossfading with a slow
  drift (blurred cover fallback), logo or title, platform/year/developer/genres/play time/
  description, animated cover shelf (windowed DOM), TV menu (Search, Settings, Desktop mode),
  "Start in TV Mode" setting. Owner's verdict: "looks good".
- Dark theme, TV-readable text.

## Explicit non-goals (unchanged)
- No ROM, BIOS, or content downloading of any kind
- No themes marketplace, video snaps, music, plugins system, or cloud sync
- No built-in emulator configuration beyond launch paths and command templates
- No account system

## Milestones
- [x] 0. Repo + scaffold
- [x] a. Settings, scanning, SQLite, basic grid
- [x] b. Launching, process tracking (Job Objects), exit combo, play time
- [x] c. Full controller navigation, on-screen keyboard, filters
- [x] d. Metadata (IGDB) and artwork (libretro-thumbnails, GameTDB, IGDB, SteamGridDB)
- [x] e. RetroAchievements
- [x] f. Performance pass (numbers above)
- [x] g. TV Mode, with SteamGridDB heroes and logos
- [x] h. Backloggd link per game (sync waits for an official API)

## Owner's proposals for future work (not started)
1. **Appearance pass** (after the basics): an app name and identity (app icon; Task Manager
   currently shows the WebView2 processes as "WebView2 Manager" until the exe has proper
   branding), platform and emulator icons inside the UI, animations, general visual polish.
2. **Compressed disc formats** ("keep in mind"): the owner may convert ~117 GB of disc images
   (PS2 alone ~90 GB) to CHD (PS1/PS2/Sega CD/PSP, via chdman), RVZ (GameCube/Wii, via Dolphin)
   or CSO, likely saving ~40-50 GB. Emulators and the scanner already handle these formats;
   **before converting, add CHD/RVZ/CSO reading to the RetroAchievements hasher** and verify
   hashes match the originals. chdman/maxcso downloads need the owner's OK.
3. **Switch and other platforms** (PS4, Xbox, Xbox 360, Saturn folders exist): add when
   emulators are set up; verify CLIs and formats against official docs.
4. **Backloggd sync** of backlog/played status, only if Backloggd publishes an official API.
5. **Installer + code signing**, if the app is shared publicly (SmartScreen/AV warnings).
6. Considered and deferred: replacing WebView2 with a native UI toolkit (see Tech stack).

## Known limitations
- RetroAchievements can't read `.chd`/`.rvz`/`.cso`/`.gcz` yet; 3DS has no achievements.
- Title-based matching (SteamGridDB, RA "no achievements" note) can miss games listed under
  other names; manual pickers / Fix match cover IGDB and art.
- Backloggd: link only. MobyGames: not used.

## How to work (updated)
1. Propose architecture/plans before large changes; build in runnable, testable milestones.
2. Before writing integration code for a new service, read its current API docs and summarize
   auth, rate limits and terms for the owner.
3. Unit tests for normalization, scanning rules, multi-disc grouping, template rendering,
   hashing and service clients (fake HTTP); ignored live tests run against **copies** of the
   real library DB, never the live one.
4. Keep the README current (setup, where to get each key, how to add a platform, performance).
5. Commit at sensible points with clear messages.
6. **How the owner runs the app:** directly from
   `C:\Emulator files\src-tauri\target\release\KaneegaBell.exe` (no installer). After changes
   they should try, run `npm run tauri build -- --no-bundle` and tell them it's ready. The build
   can't overwrite the exe while it runs: check for a running `KaneegaBell` process first and
   ask them to close it. Never drive or launch the app while they're using it.
7. Never download game content; confirm before destructive or outward-facing actions.
