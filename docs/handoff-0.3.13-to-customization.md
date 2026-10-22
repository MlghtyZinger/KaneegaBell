# KaneegaBell: handoff for the next session

Continue work on KaneegaBell (KB), a Tauri v2 / Rust / SolidJS emulator frontend in
`C:\Emulator files`, branch `main`. The owner wants to start the next milestone here.
Read `CLAUDE.md`, then `CLAUDE.updated.md`, then `CLAUDE.new1.md` (later decisions win),
and the README. This handoff updates their older release state.

## Published release: 0.3.13

- Published and verified: https://github.com/MlghtyZinger/KaneegaBell/releases/tag/v0.3.13
- Source commit: `cf29965` — `Add KB, Wii and PS3 TV menus with console audio`.
  Main was pushed; annotated tag `v0.3.13` was pushed. Do not move or recreate that tag.
- The signed installer, `.exe.sig`, and `latest.json` are uploaded. The `updates` branch
  also serves 0.3.13. An authenticated download through the exact updater URL matched the
  signed local installer by SHA-256, and the manifest's public signature matched its `.sig`.
- Installer: 9,636,275 bytes; SHA-256:
  `e91dec03af230ed4e9ae48a087bb09fd3e1084791e37cb701b5992344f22bea9`.
- Signed outputs are under `C:\Emulator files\release\v0.3.13\`.
  Do not install over or modify the owner's installed copy.

## Local changes that MUST be preserved

The working tree is intentionally dirty after publishing:

1. `tools/release/release.ps1`: a small verification fix, not yet committed. The release
   uploaded successfully, then its final verification failed because the owner's Windows
   PowerShell could not find `Get-FileHash`. The replacement uses .NET SHA-256 directly.
   It parses successfully, and independent verification using the same .NET API passed.
2. `docs/release-0.3.13.md`: updated to record successful publication/verification.
3. This handoff file is new and uncommitted.

An attempt to commit and push the first two follow-up files was rejected by automatic
approval review: it considered the follow-up outside the authorization for the completed
release. No part of that rejected command ran. The owner requested this handoff instead
of approving the follow-up. **Do not retry, indirectly push it, or assume permission.**
Get explicit approval before committing/pushing that follow-up. It does not require a new
installer or a moved tag. Preserve these changes while working on the next milestone.

The release helper already committed in `cf29965` also removes destructive
`git checkout -- .` cleanup on test/build failures and forced temporary-worktree removal.
Keep those protections.

## Next milestone: u — customization

The locked order is: completed TV update (0.3.13), then **u customization**, then
**v achievements window**. One milestone per release. Do not expand console styles or
move achievements ahead of customization without the owner's explicit decision.

Read the exact u entry near the end of `CLAUDE.new1.md`. Intended work includes:

- Library sorting: title, last played, play time, release year, date added.
- Start tab; hide/reorder platforms; hide empty platforms.
- Folding platform-maker groups (Nintendo / PlayStation / Sega / Microsoft), remembered
  open state, controller A to open/close; consider slimmer expanded-sidebar rows.
- Controller glyph selection: auto / Xbox / PlayStation / generic; text size.
- Settings export/import, excluding keys and other secrets.

Some of this may already exist. Inspect Settings, library state, navigation and persistence
before proposing additions. The older section 7 is broader than the later u decisions.
In particular, the owner **scrapped the theme editor, color/border customization and
`.kbtheme` import/export** on 2026-10-20. Do not revive them. The built-in styles stay.
Propose a focused milestone plan and 2–3 visual options where needed, then obtain the
owner's approval before a large implementation. Do not infer a new version number yet.

## What 0.3.13 contains

- TV-only KB Showcase replaced the abandoned Low shelf layout: game-logo rail, large
  selected 3D case, platform filter including All platforms, short local summaries,
  appropriately sized descriptions, and More… with the full game information.
- Cached LaunchBox gameplay screenshots (`art/q`, protocol `q`, optional `GameRow.screenshot`)
  in a 16:9 TV frame. Preserve older games' proportions/HUD. LaunchBox image kinds version
  is now 5, so it performs one metadata refresh to retain screenshots.
- Switchable KB / Wii / PS3 menus, remembered via `fx.tvMenu`.
- Wii: 4×3 channel grid, channel-opening screen, transitions, clock dock, D-pad access to
  bottom buttons, right-stick hand pointer, real menu effects and looping menu music.
  `menu_wii_theme.mp3` plays for 74.208 seconds and loops. It follows TV sound/volume and
  pauses for intros, hidden windows, gameplay and other styles; releases its source on exit.
- PS3: console crossbar, aligned game column, large selected tile and compact following
  rows, logo-over-background thumbnails, larger console icons, 10-second blue ribbon wave
  cycle, 42 drifting sparkles and menu effects. No platform pop-up; persistent platform label.
  PS3 has **no continuous background music**, per the owner's explicit clarification.
- PS3 footer clipping was caused by focus scrolling an overflow-hidden surface.
  `overflow: clip` prevents it; preserve that fix and the shared XMB alignment axis.
- Supplied recordings from `audio files` are the default startup sounds, including both
  PSP parts. Their original files were not modified. Full recording duration is respected.
- TV code/CSS are lazy-loaded; desktop layout remains separate.

Key implementation files: `src/views/TvMode.tsx`, `ConsoleMenus.tsx`, `TvDescription.tsx`,
`tvShowcase.ts`, `consolePointer.ts`; `src/styles/tv.css`, `console-menus.css`;
`src/input/actions.ts`, `src/state/fx.ts`, `src/fx/Intro.tsx`, `audio.ts`;
`src-tauri/src/input/gamepad.rs`, `src-tauri/src/launchbox/mod.rs`, art/cache helpers.
The console feedback migration enables TV sounds/Full motion once; later choices remain
respected. Reduced/Off motion and game-running pause must continue to work.

Audio credits and conversion information: `tools/sounds/README.md`. Wii music/effects came
from danintosh/Wii-Menu-HTML; PS3 effects from JMRDev0/XMB-PS3-Icons-Sounds-Pack-.
Nintendo/Sony recordings are not CC0. No firmware was downloaded. Do not invent licenses.

## Validation and known performance deviation

- TypeScript passes; all 32 TS tests pass.
- Rust library tests: 256 passed, 15 ignored, zero failed. The signed release script also
  completed its tests before the owner entered the signing password.
- Native WebView layout/media checks: 23/23 passed, including PS3 at 720p/1080p/1440p/4K,
  footer bounds after first/middle/last console-tab focus, alignment/size, Wii dock input,
  correct music duration, actual end looping, pause/resume across styles, and exit cleanup.
  Reports: `perf-lib\lib5000\data\console-qa-sparkles-music-mp3\`.
- Full idle rerun: startup **629 ms** median versus saved **426 ms** baseline (+203 ms).
  Startup regression gate failed; all other gates passed. All scroll passes 240 fps,
  p99 4.3 ms, maximum 4.4 ms, no frames over 20 ms. Idle CPU 0.77% of one core.
  Task Manager RAM: start 108 MB, scroll 217 MB, game 110 MB, after game 139 MB.
- Earlier 754 ms run was during the owner's CPU-intensive game; an intermediate run
  lost its local DevTools connection and was incomplete. Do not present it as a passing run.
- Owner explicitly said to investigate for about five minutes, then publish if there was
  no quick fix. This accepted deviation applies to 0.3.13, not all future releases.
  **The baseline was not changed.**
- Investigation found current/previous library screenshot queries both around 11–19 ms;
  warm library IPC ~25 ms. One cold profile spent ~296 ms before navigation, fetched JS/CSS
  within 6 ms, finished DOM loading at 59 ms and painted at 260 ms. Warm UI reloads painted
  in 28–32 ms. No safe quick app-code fix or controlled old-binary comparison was completed.
  The exact cause remains unproven; do not claim the screenshot query is responsible.
- Physical controller hardware, audible speaker output, all DPI/ultrawide combinations,
  and a real installation/update were not exercised by these synthetic-library checks.

## Working rules

- Work in `C:\Emulator files` and system temp only. Never touch `C:\Games\KaneegaBell`,
  E: game/emulator directories, registry, other apps' data, ACLs, permanent environment,
  Windows settings, or install software. Do not restructure top-level folders.
- Protected directories include build output, developer library, `perf-lib`, `release`,
  `audio files`, `tmp`, `node_modules`. Use existing build tools; do not browse developer
  library data. Previous explicit exceptions allowed intro assets and isolated performance
  tests under `perf-lib\lib5000`; they did not permit live/installed-library access.
- Ask whether the owner is playing before heavy builds/performance runs in the new session.
  Stop if they are. Check `Get-Process KaneegaBell` before builds/test launches; if open,
  ask the owner to exit it. Do not close it yourself.
- Important: sandboxed `Get-Process` sometimes showed no app while an unsandboxed check
  found the installed copy. Check the actual host process state before launching tests.
- Keep file line endings. Read before editing and explain replacements/deletions. No bulk
  formatters, dependency changes or build-config changes without approval.
- No destructive Git commands. Commit on main only when authorized. Never push/tag/publish
  without explicit authorization for that action. Ask before new outward-facing releases.
- Signing password is entered by the owner in their terminal, never in chat or by the agent.
  Never print/read the private key or store the password. Release scripts must preserve work.
- Startup <1.5 s, smooth grid, near-zero idle work, minimal game-running work; measure
  relevant changes. Keep controller-only navigation throughout new flows.
- Prefer small existing helpers/native features. No new dependencies needed by default.
- Be precise about what was actually built, tested and published. Owner was previously
  frustrated by claims of changes/builds that were not visible. Link the correct developer
  executable and distinguish it from old installers and the protected installed copy.
- An earlier PC-shutdown request was already fulfilled during release preparation; do not
  schedule another shutdown in this new session without a fresh request.

Commands are run from `C:\Emulator files`; always give the owner full paths. For the
current process only, remove stray quotes from PATH and add Node/cargo paths as documented.
Useful checks: `node node_modules/typescript/bin/tsc --noEmit`, `npm test`,
`cargo test --manifest-path src-tauri/Cargo.toml --lib --quiet`.
Build: `node node_modules/@tauri-apps/cli/tauri.js build --no-bundle`.
Full performance gate (only after permission):
`powershell -NoProfile -File "C:\Emulator files\tools\perf\perf.ps1" -Data "C:\Emulator files\perf-lib\lib5000\data"`.
Keep intros/sounds/launch effects off in that isolated test library and restore temporary
settings after media checks. For diff whitespace checks use
`git -c core.whitespace=blank-at-eol,blank-at-eof,space-before-tab,cr-at-eol diff --check`
so preserved CRLF files are not falsely reported as trailing whitespace.

Start by confirming the published state and preserving the three local files above, then
inspect what u already implements and propose the remaining customization work to the owner.
