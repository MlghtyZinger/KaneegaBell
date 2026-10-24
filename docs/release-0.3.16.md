# KaneegaBell 0.3.16 release evidence

Release candidate: f7eaa50, plus the release documentation commit.

- Includes the dedicated Achievements window and nine reviewed startup videos.
- All nine source assets and emitted Vite assets match the approved exports byte
  for byte. Full decode, media playback with audio, volume/mute, controller skip,
  custom sound timing and replacement cleanup passed. Reports are in
  `docs/intro-video-previews/bundled-0.3.16-checks`.
- Tauri development rebuild passed before publication preparation.
- The 5,000-game performance run on 2026-10-24 measured startup at 522 ms median
  (453–888 ms), versus the saved baseline of 426 ms. The raw gate fails only
  `startMedianMs`; the baseline is unchanged. This improves on the 629 ms startup
  result explicitly accepted for the published 0.3.13 line. The owner requested
  automatic publication of this candidate; no new slowdown versus that accepted
  published result was found.
- All scrolling passes were about 240 fps, p99 4.3 ms, maximum 12.5 ms, with zero
  frames over 20 ms. Idle CPU was 0.77% of one core; game CPU was 0.92%.
- Task Manager memory: 103 MB at start, 212 MB after scrolling, 109 MB during a
  game, and 138 MB after exit. All gated memory checks pass. Private-memory
  increases are warnings under the existing gate rules.

Raw performance results: `docs/perf-0.3.16.json`.
No baseline or performance tolerance was changed.
