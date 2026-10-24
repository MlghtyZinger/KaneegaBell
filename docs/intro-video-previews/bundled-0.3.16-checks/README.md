# Bundled intro verification — 0.3.16 candidate

Nine final reviewed videos are used: Konami, RDR1, GBA, GameCube, PS1, PS2, PS3,
PSP and Switch. Xbox and RDR2 were rejected and are excluded.

- `assets.json`: each packaged asset matches its reviewed export byte for byte;
  all files fully decode at 1920×1080 / 60 fps, with audio/video duration differences
  below one frame and media start offsets below 25 ms.
- `playback.json`: all nine completed in the actual SolidJS player in Edge, with
  nonzero decoded audio and correct volume. Controller skip, mute, TV volume,
  custom audio start/long-tail handling, replacement cleanup, and retired-setting
  fallback passed. The generated browser library was used; no owner library was driven.
- `bundle.json`: Vite emitted all nine exact assets and excluded the old separate
  intro OGG files. The media element keeps default audio and video on one clock,
  and playback completion drives dismissal instead of a guessed timeout.
- `../../intro-videos-0.3.16.json`: source selections, approved hashes and prior cue
  measurements (including PS1, PS2 and PSP) are preserved here.

The development copy's earlier local audio overrides were moved, without deletion,
to `src-tauri/target/release/data/intros/before-video-intros-2026-10-24`, so they
cannot replace the newly approved soundtracks. User-created overrides remain supported.
This is a local candidate; no publication was performed. The release performance gate
remains due after the owner's upcoming bug-fix pass.
