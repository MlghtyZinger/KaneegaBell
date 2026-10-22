# Revised console intro previews

2026-10-22, following the requested corrections to the second previews.

- GameCube: a purple cube tumbles along a path across three cube faces, building
  the familiar cubic G. The moving cube settles into its center on the closing
  chime. Kaneegabell naming and the KB bull appear below it.
- Game Boy: the title reads `Kaneegabell`, with the KB bull below. A white shine
  sweeps through the title and bull starting at the final sound, 2.98 seconds.
- PS3: only `Kaneegabell Computer Entertainment` appears over the wave. The
  preview uses SCE-PS3 Rodin LATIN Regular from the already available sound-pack
  archive. The typeface was loaded temporarily into the rendering page; no font
  file was installed or added to the repository or app.

All three MP4s are 1920×1080 at 60 fps, with the complete supplied recordings.
Each passed full decode, browser playback, seeking and completion. Source/export
audio envelopes correlate above 0.99999, with zero offset at 10 ms resolution.
Audio and video lengths agree within one frame. See `verification.json`.

These remain standalone review videos. Earlier previews are preserved.

The renderer uses `tools/sounds/classic-intro-scenes.js`. To reproduce this
revision, set `KB_INTRO_FONT` to the existing local Rodin TTF, then run:

```powershell
node tools/sounds/render-intro-videos.mjs --classic docs/intro-video-previews/revision-3
```

Add `--replace` only when deliberately replacing these known preview outputs.
