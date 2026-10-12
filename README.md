# Emu Frontend

A fast, controller-first library frontend for emulators (a lightweight LaunchBox/Big Box
alternative). It organizes and launches your existing game dumps with your existing emulators.
It never downloads ROMs, BIOS files, or any game content.

Stack: Tauri v2 (Rust) + SolidJS/TypeScript + SQLite.

## Setup (Windows 10/11)

Prerequisites:
- Visual Studio 2022 Build Tools with the "Desktop development with C++" workload
- WebView2 runtime (preinstalled on Windows 11)
- Rust (stable) and Node.js LTS:

```bash
winget install Rustlang.Rustup OpenJS.NodeJS.LTS
```

Then from the repo root:

```bash
npm install
```

```bash
npm run tauri dev
```

Release build (NSIS installer in `src-tauri/target/release/bundle/`):

```bash
npm run tauri build
```

Rust unit tests:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

## Status

Built in milestones. See `CLAUDE.md` for the full spec.

- [x] 0. Repo + scaffold
- [ ] a. Settings, library scanning, SQLite, basic grid
- [ ] b. Launching, process tracking, exit hotkey, play time
- [ ] c. Full controller navigation
- [ ] d. Metadata (IGDB, MobyGames) and artwork (SteamGridDB)
- [ ] e. RetroAchievements
- [ ] f. Performance pass
