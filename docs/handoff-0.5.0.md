# KaneegaBell: hand-off after the 0.4.7 + 0.4.8 build (written 2026-10-26)

Continue work on KaneegaBell (KB) in C:\Emulator files. Read first: CLAUDE.md (pulls in CLAUDE.updated.md and
CLAUDE.new1.md), `docs/plans/0.4.x release schedule.md`, **`docs/plans/0.4.0 And beyond.md` (the owner's own notes:
read them, not only the schedule's summary)**, then this file (newer where they disagree).
`docs/handoff-0.4.7.md` is superseded by this file.

## State
- Branch main. **0.4.7 + 0.4.8 (bundled at the owner's request) is built into the dev copy but NOT committed and NOT
  released.** The version in `Cargo.toml`/`tauri.conf.json` is still 0.4.6; `release.ps1 -Version 0.4.8` bumps it.
- Uncommitted source (commit these, never the owner's drafts): `README.md` (patch notes under `### 0.4.7`),
  `src/App.tsx`, `src/assets/platforms/ps3.png`, `src/components/Hints.tsx`, `src/fx/Model.tsx`, `src/fx/audio.ts`,
  `src/styles/{console-menus,fx,tokens,tv}.css`, `src/views/{ConsoleMenus,GameInfo,TvMode}.tsx`, new
  `src/views/CoverViewer.tsx` and `src/views/QuickSwitcher.tsx`.
- Drafts to PRESERVE (never stage, commit or delete): modified `tools/sounds/classic-intro-scenes.js`,
  `tools/sounds/render-intro-videos.mjs`; untracked `tools/sounds/check-rdr1-preview.mjs`, `docs/intro-video-previews/*`,
  `docs/0.4.0 handoff`, `docs/handoff-*.md`, `docs/plans/*`. Keep the stash "Intro preview drafts preserved before
  publishing 0.3.16".
- Installed copy C:\Games\KaneegaBell: never touch. Dev copy `src-tauri\target\release\KaneegaBell.exe`: check
  `Get-Process KaneegaBell` before every build and ask the owner to close it (they often have it open).
- Perf gate was run on the final build (the owner asked): passed. Start 446 ms median (first start 1472 ms, fresh exe),
  idle 0.26% of a core, scrolling 240 fps / max 4.4 ms, Task Manager 108 MB at start. Private memory read +17..36%
  in three later readings (warning only by the gate's rule; Task Manager figures were within +10%).

## What 0.4.7 + 0.4.8 contains (all designs picked by the owner from the artifact "KB 0.4.7 TV Mode")
- **KB-style switch bar (A1):** `.tv-switchbar` in `TvMode.tsx`: Favorites, Recent, Collections, All, then platforms
  grouped by maker (`MAKERS` in `state/platformGroups.ts`), platform pictures via `TabIcon`. L1/R1 move along it (`cycle`
  uses `barTabs()` in KB style); **up/down never reach the bar** (owner: only L1/R1). No name label under the selected
  platform. A "Go to…" button with the △ glyph ends the bar; the style logo button sits in the bar's own right column
  (so a wide Share glyph can't overlap). The bottom hint footer, the platform filter button and the game counter are gone
  from KB style (owner: hints live on visible buttons via `Badge`).
- **Quick switcher on △ (`QuickSwitcher.tsx`, opened with `setPlatformsOpen`):** KB panel, Wii rounded buttons, PS3
  side panel from the right. The PS3 one also lists the focused game's Start / Information / View cover (replaces
  PS3's old options panel). △ no longer toggles favorite in TV Mode (favorite lives in More… / details).
- **Style logo button:** KB = square with the bull (`themeIcon()`), Wii = rounded rectangle with the Wii platform
  picture, PS3 = square with the PS3 logo. The Select glyph sits beside it in each console's own look
  (`.console-select .select-wii|.select-ps3`, see memory "console-style-glyphs").
- **PS3 logo:** `src/assets/platforms/ps3.png` replaced by the owner's logo (white mask, widened and thickened to match
  PS2's weight). Used everywhere.
- **Cover viewer (`CoverViewer.tsx`, styles `.cv*` in `fx.css`):** full-screen case over its blurred cover; right stick
  turns freely (one sample per frame from the backend's `right-stick` event, step actions ignored for 300 ms after any
  stick sample so a stale step can't snap the case back); R2/L2 or wheel/Home/End zoom, gliding (rAF toward a target),
  up to 16×; d-pad/left stick moves; R3 flips; ✕ resets; ○ closes. Top-right buttons (Front/Back, −, %, +, Reset, Close)
  carry the glyphs; the game's **logo is at the top left** (owner's trial: "if I don't like it, remove it"). Move ticks are
  muted while it's open (`movesMuted` in `fx/audio.ts`, checked in `App.tsx`). Opened from KB's "Cover" button, a click
  on the case, the PS3 △ panel, and **the details page's "View cover" button (replaced Collections…)**. Not built: the
  small "where you are" map from the mockup. Zoom cap, logo and the map are open for the owner's verdict.
- **TV fixes:** stick turning smooth; disc-before-cover fixed by animating the lid only when it opens
  (`.m3.open .lid` transition in `fx.css`: a case's depth changes when its spine loads and the animated lid lagged
  behind the disc tray; verify with real covers, the mock library has none); shine clipped to the cover (`overflow: hidden`
  on `.bf/.lf`); titles' line-height 1.25 (descenders). PS3 background music was left as is (owner's decision).
- **Wii channel screen:** buttons use fixed directions (← Wii Menu, → Start, ↓ Game information, ↑ back to Start), no
  `NavScope` there; "Game information" stays centred under both buttons when focused (needed a more specific CSS rule
  because `html.nav-mode :focus` was moving it).
- **Console looks for overlays:** `TvMode.tsx` sets `html[data-tv-style]`; `tokens.css` re-themes colours for `wii` and
  `ps3`; `console-menus.css` styles `.panel/.overlay/.btn` (details page, search keyboard, menus, Settings). One blue Wii
  focus ring (the app's ring is removed on the Wii dock). PS3's bottom-left key row is gone.

## To verify with the owner first (I could not): real controller, real covers/logos
1. Disc no longer shows before the cover when scrolling disc platforms (PS1/PS2) with real covers.
2. Right-stick turning feels smooth in KB case and in the cover viewer; left→right no longer snaps.
3. Wii search (on-screen keyboard) look; it only opens with a controller. Check the focus ring there too.
4. Cover viewer logo at top left (keep or remove); zoom feels smooth; 16× cap enough.
5. Reply to the owner's question left open: "remove sound effects when moving up or down" was read as inside the cover
   viewer only. If they meant the game list too, remove move ticks there as well.
Then: commit source only with the Co-Authored-By line, `git stash push -u`, run
`tools\release\release.ps1 -Version 0.4.8 -Publish -Yes` with `run_in_terminal` (owner types the key password in that
tab), then `git stash pop`. Patch notes are already under `### 0.4.7` in the README; **rename that heading to
`### 0.4.8`** (it is the one release) and fold in anything added later (keep it concise: big features only).

## Next: 0.5.0 Manuals (see the schedule)
A per-game manuals viewer with animated page switching and zooming (transform and opacity only, follows the animation
levels, stops when a game launches). Source undecided: the owner's own files first (recommended), optionally a free
source with checked terms (earlier note: Retrokit via the Internet Archive was allowed as an on-demand download, "not game
content"). KB never downloads game content. The cover viewer's zoom/pan code can be reused. Mockups first (Design
artifact), the owner picks, then build. Perf gate: Run.
Parked, ask before picking up: the platform sidebar maker rows (mockup board 3 in "KB 0.4.5 Controls Mockups"); the
PS3 style's remaining hints still use normal glyphs, if any remain after the console-glyph rule.

## Lessons (follow them)
- Anything visual: a mockup at real size first; the owner picks. When they say "design" for something, they mean several
  options to choose from, not one (they rejected a single switch-bar design). They ask for inspiration from other apps and
  consoles. Redraw with the app's real icons (`src/assets/platforms/*.png`, white masks).
- Read the owner's own notes (`docs/plans/0.4.0 And beyond.md`) before saying a bug is fixed: the disc bug needed a second
  fix because the first guess was wrong. Say plainly what could not be tested.
- Keep messages short. Build, then say it's ready. Check for a running KaneegaBell first.
- Don't remove things the owner didn't ask to remove; say what a change takes away (e.g. △ favorite, Collections… button).
- The Vite preview caches edited files: if a change doesn't show, `touch` the file and reload. Key events: the browser tool
  can't send `[`/`]`; dispatch a `KeyboardEvent` through `javascript_tool` instead. Pad-only UI (glyph badges, the search
  keyboard) can't appear in the mock; insert the element by script to check its look.

## Working rules (unchanged)
- Build: `npm run tauri build -- --no-bundle` (PowerShell PATH: C:\Program Files\nodejs; C:\Users\watch\.cargo\bin).
  tsc: `& "C:\Program Files\nodejs\node.exe" node_modules\typescript\bin\tsc --noEmit`. Preview: `preview_start ui-mock`
  (Vite on 1420, mock IPC); Esc skips the intro, F10 toggles TV Mode, g = menu style, f = △ (quick switcher), / = search.
- Perf gate: `powershell -NoProfile -File tools\perf\perf.ps1 -Data "C:\Emulator files\perf-lib\lib5000\data"`; only for
  releases marked "Run" in the schedule or startup/grid/idle changes.
- Commit messages end with the Co-Authored-By line of the model in use. Commands given to the owner include the full path.
