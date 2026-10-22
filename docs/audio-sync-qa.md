# Intro synchronization, PS3 music and compact folding rows

2026-10-22, uncommitted developer changes; version remains 0.3.13.
Owner selected Compact folding rows and requested all intro audio checked plus
music in the PS3 TV menu. The owner confirmed they were not playing and builds
were fine. This supersedes the earlier preference for PS3 navigation sounds only.

## Findings and changes

The assets already preserved complete recordings, but several visual cue offsets
still followed older shortened tracks. The host also started its exit timers
before the animation frame that began playback, and asynchronous loading could
finish after an intro was cancelled. Audio readiness, playback, visual activation
and exit scheduling now share the start frame; teardown cancels the frame,
timers and owned source. Rockstar's shot timer starts with playback too.

| Intro | Decoded duration | Cue alignment reviewed |
|---|---:|---|
| Konami | 10.446 s | Both four-note phrases and both wordmark flashes |
| Rockstar | 11.077 s | Shotgun onset about 5.08 s; previously 4.05 s |
| GBA | 4.760 s | Letter landings from about 0.55 s, final chime about 3 s |
| GameCube | 10.124 s | First cube movement 1.19 s; logo landing about 5.24 s |
| PlayStation | 21.891 s | First phase bloom; second phase around 13 s |
| PS2 | 16.632 s | Black transition/title around the final 14.63 s chime |
| PS3 | 6.687 s | Existing glow/name timing follows the crescendo around 4 s |
| PSP | 6.165 s | First line 0.2 s; second-part white flash 3.95 s |
| Xbox 360 | 7.863 s | Ribbons begin with audio; orb/flash about 3.77 s |
| Switch | 0.828 s | Snap lands around the 60 ms click, rather than 600 ms |

Original recordings and bundled intro assets are unchanged. Custom override files
retain full duration; their arbitrary musical contents are not automatically synced.

PS3 music is `oggs/bgm.ogg` from the existing credited
[PS3 sound pack](https://github.com/JMRDev0/XMB-PS3-Icons-Sounds-Pack-), converted
with the existing FFmpeg to `src/assets/sounds/menu_ps3_theme.mp3`.
It decodes to about 134.374 s, loops, follows TV mute/volume, pauses for intros,
hidden windows and gameplay, swaps source when changing style and releases it on
TV exit. This is that pack's BGM, not an assertion about stock PS3 firmware music.
No other assets from the archive were extracted into the app.

## Completed browser checks

- TypeScript passes; all 34 TS tests pass (32 existing, two maker-group tests).
- Browser-generated library: 20 checks passed, no runtime errors.
- All ten intro buffers matched their decoded recording lengths. Audio contexts
  were running at start; measured CSS/audio start differences were 0.1–28.8 ms.
- Shotgun pellet insertion measured 5,085 ms against its scheduled 5,080 ms.
- PlayStation stayed in its first phase at 5 s, entered its second phase after
  13 s, and exited after the full 21.891-second recording, without an early cut.
- Skips stop the owned source. Cancelling during delayed loading produces no
  late intro playback. Switch's 60 ms snap and natural exit were checked.
- PS3 music decoded and played; seeking near its real end looped correctly.
  Mute, volume, intro and simulated visibility pauses/resume passed; Wii/PS3
  source switching and TV exit source release passed.
- Controller A folds groups without losing the group focus; platform selection
  and remembered open state after returning from TV passed. Sidebar bounds
  checked at 720p, 1080p and 4K with synthetic data.

Tools: `tools/sounds/check_intro_audio.py`, `tools/perf/intro-audio.mjs`.
Raw browser report/waveform/screenshots for this run:
`C:\Users\watch\AppData\Local\Temp\kb-audio-sync-a85fe06bf13748a48fa4523dd478e4d2`.

## Developer build and native checks

The developer executable built successfully at
`C:\Emulator files\src-tauri\target\release\KaneegaBell.exe`.
The local environment had Node but no npm, so the build used a system-temp Tauri
hook override running the existing TypeScript/Vite commands directly. No saved
project build configuration or dependencies changed.

`tools/perf/audio-native.mjs` passed 15 checks in the actual release WebView2,
using a newly created system-temp SQLite profile with 80 synthetic games:

- All ten complete intro buffers decoded; audio/animation starts differed by
  3.2–20.8 ms in the final native run.
- Packaged PS3 music decoded to 134.373896 s and looped at its actual end.
- All four maker groups appeared; folding and TV exit music cleanup passed.
- Sidebar layout bounds passed at 1280×720, 1920×1080 and 3840×2160.

The browser and native reports are preserved in `docs/qa/`. The final native
profile/screenshots are under the temp directory above, in `native-verified`.
Native test instances and the preview server were closed after verification.
The first native runs exposed test harness setup errors (page instrumentation,
saved settings requiring reload, and a bounds expression); the final run passed.
An early synthetic profile started the app's normal metadata job; the reusable
native harness now disables background providers for subsequent isolated runs.

Rust library tests: 256 passed, 15 ignored, zero failed on the host. An initial
sandbox run failed temporary-file/process tests with Access denied; the host run
resolved those environment failures. No Rust implementation changes were needed.

These are waveform and runtime timing checks, not a physical speaker/controller
listening test. Hardware listening and arbitrary user-provided sound alignment
are not covered.
The full performance gate has not been rerun; the saved baseline is unchanged.
No installer, publication, commit or push has been performed. Existing release
follow-up files and installed/developer library data remain preserved.
