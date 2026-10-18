# KaneegaBell milestones, part 1: building the app (milestones 0 and a–h)

Everything built from an empty folder up to the first public build, before the app was renamed
KaneegaBell and shipped as v0.2.0. The app was called "emu-frontend" back then.

**Goal:** a fast, controller-first library app for the owner's own game dumps and emulators, like
LaunchBox/Big Box, but small and quick, with performance as the top priority. It never downloads
games, BIOS files or other game content.

**Tech chosen at the start:** Tauri 2 + Rust for the app, SolidJS + TypeScript for the screens
(drawn by WebView2, which Windows already has), SQLite for the library.

---

## 0. Repo and scaffold
- Installed Rust and Node, created the git repo, and set up Tauri 2 + SolidJS + Vite.
- Wrote an architecture proposal (modules, database layout, how launching and process tracking work)
  after a read-only look at the real library in `E:\Emulator Games` and the emulators in `E:\Emulators`.

## a. Settings, scanning, SQLite, basic grid
- Settings for emulator paths, the RetroArch cores folder, game folders, and each platform's core and launch template.
- **Scanner:** runs in the background; only looks at new or changed files; each platform has a list of
  file types it accepts (saves, save states and patches are never listed); missing files hide a game
  instead of deleting it.
- File names cleaned into titles, with real-world cases as tests (No-Intro, GoodTools, scene
  releases, 3DS title IDs, "Zelda, The", hack and translation tags).
- **Multi-disc games** grouped by "(Disc N)"; KB writes its own `.m3u` playlist in its data folder
  (never next to your games); files a `.cue` or `.m3u` refers to are hidden.
- **Mixed folders:** GameCube vs Wii from the disc header; PS3 disc folders and RPCS3-installed games
  read from PARAM.SFO.
- A virtualized grid of title cards, with platform tabs.

## b. Launching, process tracking, exit combo, play time
- Launch templates are split into arguments first, then the paths filled in, with no shell, so names
  with spaces or "&" can't break.
- **Job Objects:** the emulator and any processes it starts are tracked as one group, without polling.
- **Controllers moved to SDL3** (from gilrs), because gilrs couldn't hear the controller while an emulator had focus
  and couldn't see native PlayStation pads.
- **Exit combo** (default: hold Select + Start for 1 s): asks the emulator to close first (so it
  saves), then ends it after 4 s.
- Play time and last played are tracked; a crash mid-game is cleaned up on the next start.

## c. Full controller navigation
- Every screen works with a controller alone: tabs on LB/RB, jump by letter on LT/RT, search with an
  on-screen keyboard, favorites, genre filter, card size on the right stick.
- Dropdowns became controller pickers, and an in-app file browser replaced the Windows dialog.
- PlayStation button glyphs when a PlayStation pad is used.

## d. Game info and artwork
- **Covers:** libretro-thumbnails (no account needed) matched by dump name, then cleaned title with
  region preference; GameTDB by the game's own ID (first choice for PS3).
- **IGDB** (optional, the user's own key): description, release date, developer, publisher, genres,
  "Fix match" screen; manual picks are never replaced.
- **SteamGridDB** (optional key): backgrounds (heroes) and logos for TV Mode, cover pickers.
- **MobyGames dropped:** its API became paid.
- Small WebP thumbnails served by a custom `thumb://` address; everything cached so the app works offline.
- Keys stored in Windows Credential Manager, never in the database or the repo.

## e. RetroAchievements
- Games identified by RetroAchievements' own fingerprint (hash), ported to Rust and checked against
  real dumps: SNES, N64, GBA, DS, PS1, PS2, PSP, Sega CD, GameCube, Wii (including WBFS), PS3 folders.
- Game details show progress, points, hardcore count and every achievement with its badge.
- "Not recognized" explains why, listing the versions RetroAchievements accepts (other regions, or an
  official "RA patch").

## f. Performance pass
Measured on a generated 5,000-game library (all with covers):

| Target | Result |
|---|---|
| Start in under 1.5 s | ~530 ms |
| Smooth 60 fps scrolling | 240 fps, no frame over 20 ms |
| Idle CPU near 0% | 0.3–0.9% of one core |
| Background scan | 5,000 games in ~0.3 s |
| While a game runs | grid unloaded, window minimized, WebView2 memory set to low |

The library generator and `tools/perf/perf.ps1` were added so these can be measured again any time.

## g. TV Mode
- A fullscreen Big-Picture-style mode, separate from the lean desktop mode: the focused game's
  background crossfades with a slow drift, with its logo or title, facts, description and an
  animated cover shelf. TV menu with Search, Settings and Desktop mode. "Start in TV Mode" setting.
- L3 (or F10) switches between desktop and TV Mode.

## h. Backloggd
- Backloggd has no public API and scraping isn't allowed, so each game gets a **Backloggd ↗** link
  (using the IGDB slug, or a search by title). Syncing waits for an official API.

---

**Platforms at the end of part 1 (12):** SNES, N64, GBA, DS, 3DS, GameCube, Wii, Sega CD, PS1,
PS2, PSP, PS3, using RetroArch, Dolphin, DuckStation, PCSX2, RPCS3 and Azahar.

Next: [part 2, the releases](milestones-2-releases.md).
