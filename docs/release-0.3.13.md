# KaneegaBell 0.3.13 release preparation

Prepared 2026-10-22. **Published: [v0.3.13](https://github.com/MlghtyZinger/KaneegaBell/releases/tag/v0.3.13). Installer and updater download verified. The owner accepted the measured startup deviation.**

## PS3 sparkles and Wii music (2026-10-22, subsequent build)

Developer executable rebuilt with 42 drifting PS3 sparkles, a 10-second wave cycle
(previously 14 seconds), and the Wii menu's looping background music. PS3 retains only
navigation effects, per the owner's clarification. Music uses TV sound/volume, pauses
during intros, hidden windows, gameplay and other styles, and releases its source on unmount.
Credits and conversion details are in `tools/sounds/README.md`.

TypeScript, all 32 TS tests, the release executable build and all 23 native media/layout
checks passed after the owner closed KB. Wii music uses MP3: WebView reports the correct
74.208-second playback duration, advances playback, loops at the actual end, pauses in PS3,
resumes on return to Wii and releases its source on leaving TV. PS3 has 42 sparkles and a
10-second wave cycle; the existing four-resolution layout checks still pass.
Reports/screenshots: `perf-lib/lib5000/data/console-qa-sparkles-music-mp3/`.
Visually inspected the PS3 1080p capture. The isolated test library's sound setting was
restored and the owned test instance closed. Physical speaker output, hardware controller,
intro/hidden-window pause behavior and a fresh full performance gate were not tested in
this pass. The installer remains the earlier candidate. No Rust changes or repeated Rust
suite. The owner subsequently accepted the measured startup deviation for publication.

Files for this refinement: `src/views/ConsoleMenus.tsx`, `src/styles/console-menus.css`,
`src/fx/Intro.tsx`, `src/assets/sounds/menu_wii_theme.mp3`,
`tools/perf/console-menus.mjs`, `tools/sounds/README.md`, README credits and this handoff.

## PS3 reference refinement (2026-10-22, subsequent build)

The developer executable has since been rebuilt with a tighter PS3 composition:
console/game axis at 30% width, selected game moved up, larger selected tile,
four compact following games, previous game above the crossbar, and original animated
blue ribbon waves. Changed `ConsoleMenus.tsx`, `console-menus.css`, `tvShowcase.ts`,
its window test, and the console QA script's spacing assertions.

TypeScript, all 32 TS tests, release build and all 17 native layout/navigation checks passed.
Captured screens and report: `perf-lib/lib5000/data/console-qa-ps3-spacing/`.
Visually inspected the 1080p PS3 capture. Synthetic artwork was used; no owner library was opened.
No new full performance run: changes are confined to the TV presentation, and the earlier
startup blocker below remains unresolved. No Rust changes or repeated Rust suite in this pass.
The NSIS installer below is the earlier candidate and does not include this subsequent refinement;
the developer executable does. No publishing, commits or installer installation performed.

## Changes

- KB Showcase, Wii and PS3 TV styles, switchable without restarting.
- Owner-supplied startup recordings; console menu effects and animations.
- Larger PS3 console icons; platform pop-up removed, persistent platform label retained.
- Fixed focus-induced scrolling that clipped the lower-left PS3 controls.
- Screenshot previews, bounded TV summaries, full game information under More.

Player-facing notes: `docs/release-0.3.13-notes.md`. Version fields and Cargo lockfile are 0.3.13.

## Validation

- TypeScript typecheck passed; 32 TypeScript tests passed.
- Rust library suite: 256 passed, 15 ignored, zero failed.
- Release executable and NSIS installer built.
- Focused native WebView checks: 17/17 passed. PS3 tested at 1280x720, 1920x1080,
  2560x1440 and 3840x2160 while focusing and switching first, middle and last console tabs.
  Footer bounds, zero surface/parent horizontal scrolling, enlarged active icons, alignment,
  absent pop-up and running wave animation checked. Wii bottom controls reached using
  Down/Right/Enter through the shared keyboard/controller action path.
- Visually inspected 720p and 4K PS3 captures and 1080p Wii dock capture.
- Reports/screenshots: `perf-lib/lib5000/data/console-qa-0.3.13/`.
- Physical controller hardware, audible speaker output, real artwork quality, DPI/ultrawide
  and installed updater behavior were not checked by this isolated synthetic-library run.

## Performance blocker

Two full runs against the existing baseline, using only the approved isolated 5,000-game
library. Intros, sound and launch effects off; Full animation; desktop startup.

| Metric | Baseline | Run 1 | Run 2 |
|---|---:|---:|---:|
| Startup median, ms | 426 | 652 | 580 |
| Idle CPU, % of one core | 0.52 | 0.77 | 0.77 |
| Start Task Manager RAM, MB | 110 | 106 | 107 |
| Scroll Task Manager RAM, MB | 212 | 215 | 213 |
| Game Task Manager RAM, MB | 102 | 107 | 109 |
| After game Task Manager RAM, MB | 132 | 137 | 139 |

Both runs failed only `startMedianMs`. Run 2 is 36% slower than baseline, although still
under the absolute 1.5-second target. Run 2 scrolls at 240 fps, p99 4.3 ms, maximum 4.4 ms;
game-running CPU 0.92% of one core. Private memory warnings remain (249/444/339/368 MB for
start/scroll/game/after game); the gate uses Task Manager memory, which passed.
The baseline is unchanged. The owner subsequently accepted the startup deviation for publication if a five-minute investigation found no quick fix.
Earlier results: `perf-last.json` was subsequently replaced by the final idle rerun; `perf-0.3.13-run1.json` (run 1) in the isolated data folder.

## Earlier candidate and publication record

- Executable: `C:\Emulator files\src-tauri\target\release\KaneegaBell.exe`.
- Unsigned updater candidate: `C:\Emulator files\src-tauri\target\release\bundle\nsis\KaneegaBell_0.3.13_x64-setup.exe`
  (earlier unsigned candidate: 8,815,607 bytes). Superseded by the fresh signed release below.
- Bundle-only retry warned that its bundle-type marker was already absent after the first
  packaging attempt patched the executable. Installer creation succeeded, but updater
  behavior is unverified; use the owner's normal fresh signed build for publication.
- Source committed as `cf29965`, pushed to main and tagged `v0.3.13`; GitHub release published.
- Owner has authorized the release commit and publication following the time-limited startup investigation.
- Owner completed `C:\Emulator files\tools\release\release.ps1 -Version 0.3.13 -Publish`
  after explicitly authorizing publication, entering the signing password themselves.
  The release script was hardened to preserve working-tree changes on test/build failure and avoid forced temporary-worktree removal.
- All test app instances were closed. Owner requested PC shutdown after preparation.

## Final publication decision

The complete idle rerun measured 629 ms startup (baseline 426 ms); all other gates passed,
including 240 fps scrolling and a maximum 4.4 ms frame. See the README for full figures.
The owner authorized a roughly five-minute investigation followed by publication if no quick
fix emerged. Profiling found fast JS/CSS and library IPC, and substantial cold WebView/first-paint
overhead; no app-code fix or controlled old-binary comparison was completed in that budget.
No performance baseline changed. The startup deviation is disclosed and accepted for this release.

## Published installer verification

Signed installer: 9,636,275 bytes, SHA-256
`e91dec03af230ed4e9ae48a087bb09fd3e1084791e37cb701b5992344f22bea9`.
GitHub release contains the installer, its `.sig`, and `latest.json`. The updates branch
manifest reports 0.3.13; an authenticated raw download matches the signed local installer
byte-for-byte by SHA-256, and its public signature matches the manifest. Main and the release
tag were confirmed remotely. No installed-owner-library files were modified.

The release script failed only after upload, when its host could not find `Get-FileHash`.
Verification was completed independently using .NET SHA-256, and the helper now uses that
API for future releases. This helper/doc correction is a follow-up on main; the release tag
and signed installer remain unchanged.
