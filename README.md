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
2. **Settings → Library → ROM folders**: add each ROM folder and assign its platform.
   - Use **Auto** only for mixed folders (e.g. a romhack folder with GBA and DS games, or a
     GameCube + Wii folder). Auto decides by extension and disc headers and lists anything it
     can't place under the scan's warnings.
   - A folder assigned to GameCube or Wii accepts both; each disc is placed by its header.
   - **PS3**: assign disc-dump folders (containing `PS3_GAME`) to PlayStation 3, and use
     "Add RPCS3 installed games" for PSN/HDD titles in RPCS3's `dev_hdd0`.
3. **Scan library.** Rescans are incremental, and games whose files disappear are hidden (not
   deleted), so play time and favorites survive a drive being unplugged.

Keyboard: arrows move, `Enter` details (and `Enter` again to play), `Q`/`E` (or PgUp/PgDn)
switch platform, `Home`/`End` jump by letter, `F` favorite, `F1` settings, `Esc` back, `F11`
fullscreen.

## Controllers

| Action | PlayStation | Xbox |
|---|---|---|
| Move | D-pad / left stick (hold to repeat) | D-pad / left stick |
| Details, then Play | Cross | A |
| Back | Circle | B |
| Favorite | Triangle | Y |
| Previous / next platform | L1 / R1 | LB / RB |
| Previous / next letter | L2 / R2 | LT / RT |
| Settings | Options | Menu |
| **Exit the running game** | **hold Share + Options 1 s** | **hold View + Menu 1 s** |

The exit combo is configurable in Settings → General.

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

## Scanning rules

- Only files with the platform's extensions are considered, so saves, states, patches and
  readmes never appear.
- Files referenced by an `.m3u` or `.cue` (and `.ccd`/`.mds` sidecars) are hidden, so a game
  appears once.
- Discs named `(Disc N)` / `(Disc N of M)` / `(CD N)` in the same folder are grouped into one
  entry. A real `.m3u` always wins over grouping.
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
2. Restart the app: built-in definitions are re-seeded on every start without touching your
   settings.
3. If its extensions overlap another platform's (`.iso`, `.cue`, `.chd`…), files in Auto
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
- [ ] c. Full controller navigation
- [ ] d. Metadata (IGDB, MobyGames) and artwork (SteamGridDB)
- [ ] e. RetroAchievements
- [ ] f. Performance pass
- [ ] g. TV Mode: a switchable Big-Picture-style view with animations (desktop mode stays the
      main, lean UI)
