# Emu Frontend

A fast, controller-first library frontend for emulators (a lightweight LaunchBox/Big Box
alternative). It organizes and launches your existing game dumps with your existing emulators.
It never downloads ROMs, BIOS files, or any game content.

Stack: Tauri v2 (Rust) + SolidJS/TypeScript + SQLite + SDL3 (controllers).

## Setup (Windows 10/11)

Prerequisites:
- Visual Studio 2022 Build Tools with the "Desktop development with C++" workload
- WebView2 runtime (preinstalled on Windows 11)
- Rust (stable) and Node.js LTS:

```bash
winget install Rustlang.Rustup OpenJS.NodeJS.LTS
```

- CMake, for the SDL3 source build. VS Build Tools ships one; if `cmake` isn't on your PATH,
  point cargo at it with a git-ignored `.cargo/config.toml` in the repo root:

```toml
[env]
CMAKE = { value = 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe', force = false }
```

Then from the repo root:

```bash
npm install
```

```bash
npm run tauri dev
```

Release build. The app is `src-tauri/target/release/emu-frontend.exe`; drop
`-- --no-bundle` to also get an NSIS installer in `src-tauri/target/release/bundle/`:

```bash
npm run tauri build -- --no-bundle
```

Logs go to stderr; set `RUST_LOG=emu_frontend_lib=debug` for more detail.

Rust unit tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

## First run

1. **Settings → Emulators → "Find emulators in a folder…"** and pick the folder that holds your
   emulators (e.g. `E:\Emulators`). It fills in every emulator it recognizes and the RetroArch
   cores folder. Or set each path by hand.
2. **Settings → Library → ROM folders**: add each ROM folder. The platform is preselected
   from the folder name ("PS2", "GBA", "GameCube"...); change it if needed.
   - **Auto** works out each file's platform from its type and contents: GameCube vs Wii from
     the disc header, PS1 vs PS2 from the disc's `SYSTEM.CNF`, PSP and Sega CD from their disc
     signatures. When the contents can't decide, folder names along the path ("PS2", "PSX")
     break the tie. Anything still ambiguous is listed under the scan's warnings.
   - A folder assigned to GameCube or Wii accepts both; each disc is placed by its header.
   - **PS3**: assign disc-dump folders (containing `PS3_GAME`) to PlayStation 3, and use
     "Add RPCS3 installed games" for PSN/HDD titles in RPCS3's `dev_hdd0`.
3. **Scan library.** Rescans are incremental, and games whose files disappear are hidden (not
   deleted), so play time and favorites survive a drive being unplugged.

Keyboard: arrows move, `Enter` details (and `Enter` again to play), `Q`/`E` (or PgUp/PgDn)
switch platform, `Home`/`End` jump by letter, `F` favorite, `F1` settings, `Esc` back, `F11`
fullscreen.

## Controllers

| Action | PlayStation | Xbox | Keyboard |
|---|---|---|---|
| Move | D-pad / left stick (hold to repeat) | D-pad / left stick | Arrows |
| Select / details, then Play | Cross | A | Enter |
| Back / close | Circle | B | Esc |
| Search | Square | X | `/` |
| Favorite | Triangle | Y | F |
| Previous / next tab (platform, Settings section) | L1 / R1 | LB / RB | Q / E |
| Previous / next letter | L2 / R2 | LT / RT | Home / End |
| Settings | Options | Menu | F1 |
| Bigger / smaller cards | right stick up / down | right stick up / down | `+` / `-`, Ctrl + wheel |
| Platform sections on / off (All, Favorites, search) | R3 | RS (click) | V |
| Genre filter (with IGDB) | Share | View | G |
| **Exit the running game** | **hold Share + Options 1 s** | **hold View + Menu 1 s** | |

The exit combo is configurable in Settings → General. Every screen works with a controller
alone: D-pad moves focus to the nearest control, A/Cross presses it, and the hints bar shows
the buttons for the device you used last.

- **Text fields** open an on-screen keyboard (□/X space, ○/B delete, △/Y shift, Options/Menu
  done). A physical keyboard types into it too.
- **Search** (□ / X / `/`) searches every platform as you type; ○ / B clears it.
- **Dropdowns** are pickers you can open with A, and **Browse…** opens an in-app folder/file
  browser (A enters, B goes up, □ / X picks the current folder). The native Windows dialog
  can't be used with a controller.
- **Tabs**: All, ★ Favorites and Recent (most recently played first) appear next to the
  platforms once they have games.

Controller input uses SDL3 rather than gilrs: gilrs' Windows backend (Windows.Gaming.Input)
only delivers input to the focused window, so the exit combo couldn't work while an emulator
has focus. SDL3 reads XInput, raw input and HIDAPI (native DualShock 4 / DualSense) with
background events enabled. It is told not to change PlayStation pads' report mode or LEDs, so
emulators reading the same pad see it unchanged. When Steam Input or DS4Windows exposes a
virtual Xbox pad next to the real one, the same press arriving from both is counted once.

## Launching and play time

- The emulator is started directly (no shell) from the platform's launch template, inside a
  Windows Job Object. The session ends when the last process in the job exits, so emulators
  that spawn child processes are tracked correctly, without polling.
- While a game runs, the frontend minimizes (WebView2 stops rendering) and unmounts the
  library. When the game exits, it comes back to the front, fullscreen again if it was
  fullscreen, with focus on the game you launched.
- Exit combo / "Close game": the emulator's windows get a normal close request (so it can save),
  then the whole process tree is terminated after 4 seconds if it hasn't exited.
- Multi-disc games without an `.m3u` get one generated in the app's data folder (never next
  to your files) for emulators that support playlists (RetroArch, DuckStation, Dolphin), so
  in-emulator disc swapping works. PCSX2 and RPCS3 boot disc 1.
- Play time is the session's wall-clock duration, added when the game exits. If the frontend
  crashes mid-session, "last played" is kept but that session's duration isn't guessed.

## Covers and metadata

Everything works without accounts or API keys:

- **Box art** comes from [libretro-thumbnails](https://thumbnails.libretro.com) (the set RetroArch
  uses), matched by game name: exact No-Intro/Redump file name first, then the cleaned title
  with your region preferred, then a strict word-overlap match for loose names. Sequel numbers
  must agree, so a wrong game is never picked just because the titles look alike.
- **[GameTDB](https://www.gametdb.com)** covers (the source Dolphin and the Wii USB loaders use)
  are looked up by the game's own ID read during the scan: PS3 title ID, GameCube/Wii disc ID,
  DS/3DS game code. It comes first for PS3, where libretro has few covers, and fills gaps
  elsewhere. A PS3 game's own icon (`ICON0.PNG`) is never picked automatically; it's offered
  in the cover picker.
- Covers are fetched in the background after each scan (toggle in Settings → Library), cached
  as small WebP thumbnails for the grid plus a larger image for details, and served from disk,
  so they work offline. Games without a match are retried weekly; "Refresh" in a game's
  details fetches again, and "Change cover…" lets you pick another one (other regions, a
  search of the whole cover set, or an image file). Your choice is never overwritten.
- Each card takes its cover's shape (a SNES box is a wide card, a GBA box a square one), and a
  platform tab sizes its grid for that platform's usual cover shape. All, Favorites and search
  results are shown in one section per platform, so each row holds covers of one shape (no gaps
  inside rows); R3 / V switches to a single list.
- **Game info from IGDB (optional, free key):** descriptions, release dates, developer,
  publisher and genres, plus a genre filter in the library (Share / View / `G`). Set it up in
  Settings → Game info:
  1. Sign in at [dev.twitch.tv/console](https://dev.twitch.tv/console) with a free Twitch account
     (Twitch requires two-factor authentication).
  2. **Register Your Application**: any name, OAuth Redirect URL `http://localhost`, category
     *Application Integration*, client type **Confidential**.
  3. Open the app, copy the **Client ID**, press **New Secret** and copy the **Client Secret**.
  4. Paste both in Settings → Game info and press **Save and connect** (the keys are checked with
     Twitch before they're saved).

  The keys are stored in Windows Credential Manager (`EmuFrontend:*` entries), never in the
  database or the repo. Lookups run in the background after covers: games are searched by title
  on their platform (one search per request, at most 4 requests/second, IGDB's limit) and matched with
  the same title rules as covers; matched IGDB ids are stored, so later refreshes are direct
  lookups and details work offline. A wrong match can be fixed from the game's details
  (**Fix match…**); your pick is never replaced. IGDB is free for non-commercial use under the
  Twitch Developer Services Agreement.
- SteamGridDB (alternative covers, backgrounds and logos with your own free key) is planned.
  MobyGames isn't used: its API now requires a paid subscription.

## RetroAchievements

Optional: each game's details show its achievements, your unlocks (softcore and hardcore),
points and how rare each one is. Set it up in Settings → Game info:

1. Sign in at [retroachievements.org](https://retroachievements.org) (free account, the one your
   emulators use).
2. Open [retroachievements.org/settings](https://retroachievements.org/settings) and copy your
   **Web API Key**. It works like a password for your account's data.
3. Enter your username and the key and press **Save and connect** (they're checked with RA first,
   then stored in Windows Credential Manager, never in the database or the repo).

How games are recognized: by RetroAchievements' own fingerprint of the game file, the same
method its emulators use (the rcheevos hashing rules, reimplemented in
`src-tauri/src/achievements/hash.rs`), so file names don't matter. That's whole-ROM MD5 for
GBA, the header-stripped ROM for SNES, byte-order-normalized ROM for N64, header + code + icon
for DS, the boot executable for PS1/PS2/PSP (read from the disc's file system, `.cue`/`.bin` or
`.iso`), the first sector for Sega CD, apploader + main executable for GameCube and Wii
(`.iso`, and Wii `.wbfs` read through its block table), and `PARAM.SFO` + `EBOOT.BIN` for PS3
folders. Compressed formats (`.chd`, `.rvz`, `.cso`…) aren't hashed yet. 3DS shows no
achievements (hashing needs console keys).

- Hashing runs in the background after scans (a file is only re-hashed when it changes), then
  each hash is looked up in RA's list of known hashes for that console, cached for a week. A
  console RA doesn't currently support shows no achievements section at all.
- When RA doesn't recognize a file, the details say why: if RA has achievements for a game by
  that title, it lists the versions RA accepts (often another region, or a dump with an
  official RA patch applied, marked "RA patch"); otherwise it says no achievements were found
  for that title on that console (matched by title, so a game RA lists under another name,
  like a Japanese title, can land here). The file's hash is shown either way.
- Unlocks are re-checked when you open a game's details (at most every 10 minutes; Refresh
  forces it) and shown from the cache when offline. Badge images are cached on disk.
- RA rate-limits its API without publishing numbers, so requests are sequential and spaced
  half a second apart.

## Scanning rules

- Only files with the platform's extensions are considered, so saves, states, patches and
  readmes never appear.
- Files referenced by an `.m3u` or `.cue` (and `.ccd`/`.mds` sidecars) are hidden, so a game
  appears once.
- Discs named `(Disc N)` / `(Disc N of M)` / `(CD N)` in the same folder are grouped into one
  entry. A real `.m3u` always wins over grouping.
- Auto folders also skip what is never a game: folders named like caches, shaders, logs,
  saves, states, BIOS or firmware; disc images under 1 MB (Amiibo dumps, save data); plain
  `.gz` files (only `name.iso.gz` counts); and folders named for a platform that isn't set up
  yet ("Switch", "PS4", "Xbox"...). Those are scanned automatically once that platform is added.
  PS3 game folders and an RPCS3 `dev_hdd0` are recognized inside Auto folders.
- Entries whose files disappear are deleted unless they have play time or are a favorite. Those
  are kept hidden, so they come back intact if the files return.
- After each scan (and when a folder is removed) a notice says what changed, and Settings →
  Library lists every game added, removed, hidden or moved to another platform, by platform.
- Titles are cleaned for display (`Legend of Zelda, The - The Wind Waker (USA)` → "The Legend
  of Zelda: The Wind Waker"), with hack/translation tags kept as a subtitle.

To scan a real library from the command line without the UI (read-only):

```bash
EMU_REAL_SOURCES="E:\Games\GBA=gba;E:\Hacks=auto" cargo test --manifest-path src-tauri/Cargo.toml real_library -- --ignored --nocapture
```

## Adding a platform

1. Add a `PlatformDef` to `PLATFORMS` in `src-tauri/src/platforms.rs` (id, names, emulator,
   default core for RetroArch, extensions). If it needs a new emulator, add an `EmulatorDef`
   with its launch template, checked against that emulator's own CLI docs/source.
2. Make sure `src-tauri/src/scanner/hints.rs` maps the platform's usual folder names to the
   new id (Switch, PS4, Xbox, Saturn and others are already listed), so Auto scans pick up its
   folders and Settings preselects it.
3. Restart the app: built-in definitions are re-seeded on every start without touching your
   settings.
4. If its extensions overlap another platform's (`.iso`, `.cue`, `.chd`…), files in Auto
   folders stay ambiguous unless you add a header check in `src-tauri/src/scanner/detect.rs`.
   Folders with an assigned platform always work.

## UI development without Tauri

`npm run dev` and open http://localhost:1420 in a browser: outside Tauri, the dev build serves
mock data (`src/dev/mockIpc.ts`, 5,000 fake games; `?games=N` to change). The mock is a
dev-only dynamic import and is not part of release builds.

## Status

Built in milestones. See `CLAUDE.md` for the full spec.

- [x] 0. Repo + scaffold
- [x] a. Settings, library scanning, SQLite, basic grid
- [x] b. Launching, process tracking, exit hotkey, play time
- [x] c. Full controller navigation
- [ ] d. Metadata (IGDB) and covers (libretro-thumbnails, GameTDB, IGDB) done; SteamGridDB to do
      (MobyGames dropped: paid API)
- [x] e. RetroAchievements
- [ ] f. Performance pass
- [ ] g. TV Mode: a switchable Big-Picture-style view with animations (desktop mode stays the
      main, lean UI)
- [ ] h. Backloggd: Backloggd has no public API (checked Oct 2026), and scraping the site is out.
      Planned: an "Open on Backloggd" link per game (its pages are keyed by IGDB slugs, which the
      IGDB match already provides). Syncing your backlog/played status waits for an official API.
