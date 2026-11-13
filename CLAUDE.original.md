# Project: A fast, controller-first emulation frontend (lightweight LaunchBox alternative)

## Goal
Build a desktop game library frontend for emulators, similar in purpose to LaunchBox/Big Box,
but with performance as the top priority and a deliberately small feature set. I own all
hardware and games involved; this app only organizes and launches my existing dumps and
emulators. It must never download ROMs, BIOS files, or any game content.

Target OS: Windows 10/11 (keep paths and process handling cross-platform-friendly where cheap).

## Tech stack (use this unless you have a strong, explained reason not to)
- Tauri v2 with a Rust backend
- Frontend: SolidJS or Svelte + TypeScript (pick one and justify briefly), no heavy UI frameworks
- SQLite (via rusqlite or sqlx) for the library database
- `gilrs` in Rust for controller input, forwarded to the frontend as navigation events.
  Do not rely solely on the browser Gamepad API, because input must also work for the
  "exit emulator" hotkey while an emulator has focus.
- Images cached on disk as resized thumbnails (WebP), served to the UI via a custom protocol

## Performance requirements (treat these as acceptance criteria)
- Cold start to interactive library in under 1.5 seconds with 5,000 games
- Smooth 60fps scrolling in the game grid: virtualize the grid, lazy-load images,
  never decode full-size artwork for grid thumbnails
- Idle CPU usage near 0% and modest RAM use; no polling loops faster than needed
- Library scans and metadata fetching run in the background and never block the UI
- When a game launches, the frontend should minimize its resource use (pause rendering,
  stop animations) until the emulator process exits

## Controller support
- Every screen must be fully usable with a controller alone (Xbox, PlayStation, and
  generic XInput/DirectInput pads). Mouse and keyboard should also work.
- Clear focus highlighting, D-pad/stick navigation, A = select, B = back,
  shoulder buttons = jump between platforms, triggers = jump alphabetically
- Configurable "exit emulator" button combo (default: Select + Start held for 1 second)
  that closes the running emulator and returns to the frontend
- An on-screen keyboard for search when using a controller

## Supported platforms and emulators
Each platform maps to an emulator with a configurable command-line template.
Use these defaults, but verify the current CLI flags for each emulator against its
official documentation before hardcoding anything:

| Platform | Emulator | Default core / notes |
|---|---|---|
| Nintendo 3DS | Azahar | standalone |
| Nintendo 64 | RetroArch | mupen64plus_next |
| Nintendo DS | RetroArch | melonDS DS (or melonDS) |
| Game Boy Advance | RetroArch | mGBA |
| GameCube | Dolphin | standalone, batch/fullscreen mode |
| Wii | Dolphin | standalone, batch/fullscreen mode |
| Sega CD | RetroArch | Genesis Plus GX |
| PlayStation 1 | DuckStation | standalone, batch/fullscreen mode |
| PlayStation 2 | PCSX2 | standalone, no-GUI/fullscreen mode |
| PlayStation 3 | RPCS3 | standalone, no-GUI mode |
| PSP | RetroArch | PPSSPP |
| SNES | RetroArch | Snes9x (or bsnes) |

Requirements:
- A settings screen where I set the path to each emulator executable (and the RetroArch
  cores folder), and can override the core or the full launch template per platform
- Launch templates use placeholders like {emulator}, {core}, {rom}, {rom_dir}
- Per-platform file extension lists for scanning (e.g. .chd/.cue/.m3u for disc systems,
  .rvz/.iso for Dolphin, .3ds/.cci/.cxi for Azahar, etc.). Verify supported formats per emulator.
- Multi-disc games: group discs into one library entry; prefer .m3u when present
- PS3: support both folder-format games (detect PS3_GAME / EBOOT.BIN) and games installed
  inside RPCS3's dev_hdd0, reading titles/IDs from PARAM.SFO
- Track play time and last played (start/stop of the emulator process) - this is the only
  "stats" feature I want

## Metadata and artwork sources
All API keys/credentials are entered in settings and stored locally (OS keychain if
practical, otherwise a local config file excluded from git). Never commit keys.

1. IGDB (primary metadata): title, description, release date, developer, publisher,
   genres. Uses Twitch OAuth client credentials. Respect its rate limits.
2. MobyGames (fallback/supplement): fill gaps IGDB doesn't cover, which matters most for
   older and obscure titles. Respect its (strict) rate limits.
3. SteamGridDB (artwork): box art/grid images, plus hero and logo images if available.
   Let me choose between alternative images for a game from the controller UI.
4. RetroAchievements: show achievement lists and my unlock progress per game.
   - Match games by hash using RetroAchievements' official hashing method
     (the rcheevos approach), not by filename
   - Check RA's current supported console list. Some of my platforms (likely 3DS, Wii, PS3)
     may not be supported; hide the achievements UI for those instead of erroring.

Matching strategy:
- Normalize filenames (strip region tags, revision tags, disc numbers, etc.) before searching
- Use platform + title to query; store the matched external IDs in the database so
  future refreshes are direct lookups
- Provide a manual "fix match" screen to search and pick the correct entry
- Cache all API responses and images locally; the app must work fully offline after
  the initial fetch

Before writing the integration code, read each service's current API documentation and
summarize for me the auth method, rate limits, and terms-of-use constraints you found.

## UI
- Main view: platform list/carousel, then a virtualized box-art grid for the platform
- Game detail view: art, description, metadata, play time, achievements, Play button
- Search and simple filters (genre, favorites, recently played)
- Favorites
- Dark theme, large readable text suitable for a TV at a distance
- Optional fullscreen "TV mode" on startup

## Explicit non-goals (do not build these)
- No ROM, BIOS, or content downloading of any kind
- No themes marketplace, video snaps, music, plugins system, or cloud sync
- No built-in emulator configuration beyond launch paths and command templates
- No account system

## How to work
1. Start by proposing the architecture: module layout, database schema, and the
   launch/process-tracking design. Wait for my approval before writing large amounts of code.
2. Build in milestones, each runnable and testable:
   a. Settings + library scanning + SQLite + basic grid (no metadata)
   b. Launching emulators, process tracking, exit hotkey
   c. Full controller navigation
   d. IGDB + MobyGames metadata, then SteamGridDB artwork, with caching
   e. RetroAchievements integration
   f. Performance pass: profile against the targets above with a generated
      5,000-entry test library and report the numbers
3. Write unit tests for filename normalization, scanning rules, multi-disc grouping,
   and launch-template rendering.
4. Keep a README with setup steps, where to get each API key, and how to add a new platform.