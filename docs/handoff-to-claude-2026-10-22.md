# Handoff to Claude — KaneegaBell, 2026-10-22

The owner explicitly paused our work and requested this handoff. Do not resume rendering, implement more features, publish, or shut down the PC merely because you read this document. Wait for the owner's instruction to continue. This document records status and suggestions; it is not a new user request.

## Workspace and boundaries

- Project: `C:\Emulator files`, Tauri v2 / Rust / SolidJS, KaneegaBell (KB).
- Read `CLAUDE.md`, `CLAUDE.updated.md`, `CLAUDE.new1.md`, and the original `docs/handoff-0.3.13-to-customization.md`; later direct owner instructions take precedence over historical preferences.
- Published baseline is 0.3.13 / commit `cf29965`. No new version was chosen. No changes from this session were committed, pushed, installed, or released.
- Working tree is intentionally dirty. Preserve all existing changes, including the release verification fix and release notes. Do not reset, clean, overwrite the installed app at `C:\Games\KaneegaBell`, or touch real library data.
- Earlier automatic approval review rejected committing/pushing the release follow-up. Do not retry without fresh explicit authorization.
- Owner previously confirmed they were not playing and heavy builds were fine. Do not install software, dependencies, or fonts for these previews. No subagents were used in this batch.

## Broader update status

The customization milestone is still u, followed by v achievements; no theme editor or `.kbtheme` feature. The owner chose **Compact folding rows**, including in the latest visualization interaction. The implementation already includes desktop maker groups, remembered folding, controller A toggling and stable focus, while preserving the flat shoulder-button platform cycle. Remaining customization items are planned, not implemented: sorting, start tab, platform visibility/order, glyph override, desktop text presets, settings transfer. Details: `docs/customization-plan.md`.

The owner asked to check every intro's audio sync and add PS3 menu music. App-level startup/timer cancellation and cue alignment were fixed. PS3 BGM was added from the previously credited sound pack and follows mute/volume, visibility, gameplay, intro and style changes. This supersedes the old handoff's statement that PS3 should have no BGM. Prior verification: 34 TS tests, 20 browser checks, 15 native WebView2 checks, and 256 Rust tests passed (15 ignored). These were run before the later standalone preview changes, not rerun for the current batch. See `docs/audio-sync-qa.md` and `docs/qa/`. Developer executable was built at `src-tauri/target/release/KaneegaBell.exe`; installed copy remains untouched.

Owner wants ScreenScraper as main metadata provider using RomM's process, without accounts or keys. Research is complete; **no provider replacement implemented**. RomM supplies app developer credentials in official images but still documents a ScreenScraper user account. Guest access is conditional; KB has no authorized developer credentials. Do not extract RomM's keys or promise reliable account-free access. Findings and primary links: `docs/screenscraper-romm-research.md`.

## Current active work before the pause: standalone intro videos

The owner asked us to create new videos using their existing recordings and make them look as close to the originals as possible, with KB naming and logo. These are review MP4 previews, **not integrated into app startup**. Preserve original MP3 files in `audio files`.

Owner feedback to retain:

- Match original colors, typography, staging, motion and audio phrases; merely shortening a video is insufficient.
- Avoid dead pauses such as the old PS2 0:10–0:14 gap. Audio may be cut thoughtfully, but the owner later allowed a few extra seconds to preserve the original phrasing.
- Check actual reveal/impact timing against audio, not just equal stream lengths.
- GameCube: purple blocks form a real cubic G; final inner cube must bounce. The corrected shape was accepted.
- Game Boy: mixed-case `Kaneegabell`; shine on the final sound effect.
- PS3: `Kaneegabell computer entertainment` in the PS3-style font.
- PS1: original-like colors; established KB logo on final screen; remove bottom KB footer. Earlier invented logos were rejected.
- PS2: correct original-style font and pacing.
- PSP: a distinct blue/purple waves-only stage, smooth transition, yellow/gold flare with reveal, improved font. Final lettering must be **KB**, with **Kaneegabell Computer Entertainment** underneath.

### Accepted/latest previews — preserve them

- `docs/intro-video-previews/sony-revision-5`: latest PS1/PS2/PSP; owner said “Good now the next batch.” PS1 10.0 s, PS2 9.6 s, PSP 6.165 s. README and verification JSON contain cuts and checks. Actual measured cue differences: PS1 about 30 ms, PS2 about 6.7 ms, PSP about -16.7 ms. Do not rerender without the original temporary font setup; otherwise fonts may regress.
- `docs/intro-video-previews/gamecube-bounce`: corrected accepted G with final cube bounce.
- `docs/intro-video-previews/revision-3`: prior GBA and PS3 corrections, plus superseded GameCube version. Check the parent preview README for the chronological chain.
- Historical Sony revisions 2–4 and `remaining-classic` are retained for reference, not the latest approved Sony work.

### Next batch in progress: Xbox 360, Switch, Konami, Rockstar

We interpreted “next batch” as these four and told the owner. Scene code and renderer have been edited, syntax checked, and **eight still keyframes rendered/visually inspected**. Full revised MP4s have **not** been rendered, decoded, played, or audio-verified yet. There is no `next-batch-revision-2` output folder yet. No render process was running at the pause; the last keyframe command finished normally.

Changes currently in `tools/sounds/classic-intro-scenes.js`:

- Xbox: silver sphere resolves from 1.13 to 3.75 s, green spiraling ribbons, green KB bull on the sphere replacing the X, pale background, green Kaneegabell name. Ring/flash and title midpoint at 3.75 s. Full 7.863 s recording. Last still looked readable; early sphere has a rotating bull and green rings. Native Segoe UI font approximation.
- Switch: red field, white controller halves; right half lands with the 60 ms click, Kaneegabell underneath. Removed extra footer bull. Full 0.828 s audio plus silent readable hold to 1.8 s. Native Arial.
- Konami: silver ribbons on black assemble, transition to orange upper/red lower ribbons on white. Italic bold Kaneegabell. Removed the old 5.33 s reset/repeat. Use one complete phrase, source 0–5.25 s with 180 ms end fade. Color transition midpoint 2.50 s. Native Arial Black italic approximates the original Univers Bold Italic.
- Rockstar: engraved brass shotgun shell heads with KB/name markings and red rim lighting; close at about 4.11 s; blast flash at 5.08 s. Red bull/name reveal moved forward from 6.25 s to 5.22–5.60 s. Full 11.077 s recording. Native Arial Black. Still inspection shows the shell heads and final red bull; no full-motion QA yet.
- Shared `mark` and `tracking` helpers now multiply existing alpha, preserving parent fades. Accepted video files themselves were not modified.

Changes in `tools/sounds/render-intro-videos.mjs`:

- New `--next-batch` selects just these four from the existing remaining-scene list; file names remain `kb-<key>-classic.mp4`.
- New optional `holdDuration` pads audio with silence via FFmpeg `apad` for Switch, with output `-t` limiting total to 1.8 s. Reports distinguish source duration from hold duration.
- Konami uses temporary edited WAV; source files remain unchanged.
- Cue metadata added for each of the four.
- Existing `--sony-revision`, `--remaining`, `--classic`, `--only=<key>` and `--replace` modes remain. Without `--replace`, existing videos cannot be overwritten. Be careful: an `--only` run overwrites verification JSON with only that run's entries.

## Resume path, only after the owner resumes

1. Recheck current diff and any applicable project instructions. Revisit motion/reference fidelity if needed; stills alone cannot establish it.
2. Render four videos into a **new** folder, suggested `docs/intro-video-previews/next-batch-revision-2`:

   ```powershell
   & 'C:/Users/watch/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node.exe' tools/sounds/render-intro-videos.mjs --next-batch docs/intro-video-previews/next-batch-revision-2
   ```

3. Full-decode and browser-play/seek/end each MP4. Check actual encoded cue frames against source audio envelopes: Xbox 3.75 s flourish, Switch .06 s click, Konami 2.50 s color midpoint, Rockstar 5.08 s blast. Verify zero audio offset, natural cut tails and Switch's silent hold; do not claim listening checks that were not performed.
4. Write a batch README, add checks to verification JSON, update parent preview README to clearly identify latest outputs (it currently has several historical entries awkwardly labeled “latest/current”). Preserve earlier files.
5. Show the four local MP4s inline for owner review. Do not integrate into app or publish automatically.

## Runtime and QA artifacts

- Existing Node: `C:\Users\watch\.cache\codex-runtimes\codex-primary-runtime\dependencies\node\bin\node.exe`.
- Existing Python: same runtime root, `dependencies\python\python.exe`; numpy and Pillow available.
- Playwright: `C:/Users/watch/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright`.
- Browser: `C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe`, headless. In Codex sandbox, launching it required approved host execution.
- `ffmpeg` and `ffprobe` already on PATH. Renderer produces 1920×1080, 60 fps H.264/AAC, fast-start MP4.
- Local still QA helper: `src-tauri/target/next-batch-qa.cjs` (ignored build directory). Eight frames: `next-<scene>-<time>.png`. Contact sheet: `src-tauri/target/next-contact.jpg`. These are temporary QA files, not delivery assets.
- For previously accepted Sony font handling, renderer accepts temporary `KB_WORDMARK_FONT` and `KB_MONOGRAM_FONT` FontFaces; do not install/bundle raw font files. Current four use native fonts and need no such variables.
- Source scene renderer uses the established bull artwork in `src-tauri/icons/icon-master.svg`.

Reference research already consulted: Microsoft's Xbox 360 logo guidelines (`https://download.microsoft.com/download/e/5/4/e5498215-0265-4095-819f-684147b4d8e7/Xbox360_Logo_Guidelines.pdf`), Konami logo history (`https://www.gamestone.co.uk/gradiushomeworld/guides_konami_logo_history.php`), and original Rockstar shotgun intro capture (`https://www.youtube.com/watch?v=P9OHio9jVS0`). Native font choices are approximations; do not claim exact original typography or perfect reproduction.

The owner previously requested shutdown after finishing earlier work. Their latest instruction is to pause and prepare this handoff; do not treat this handoff as authorization to shut down now.
