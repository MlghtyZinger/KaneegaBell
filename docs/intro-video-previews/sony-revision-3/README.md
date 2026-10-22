# Revised PlayStation staging, logos and lettering

2026-10-22. Addresses the request to improve the original sequences rather than
only shorten them. The previous 7.8 s / 7.6 s / 6.165 s audio edits are retained;
the visual staging and lettering are revised. Earlier previews are preserved.

- PS1: the stacked bull design is replaced by a dimensional red K over a
  flattened yellow/teal/blue B. The name uses Zrnic lettering. The bottom KB
  footer is removed; the licensing layout remains.
- PS2: the camera accelerates through the towers after the company line fades.
  The game-launch screen is a single softly glowing name in Zrnic, replacing
  the hand-drawn KB and name outlines. Its reveal briefly resolves from blur.
- PSP: the company line fades out before an extended dark blue/violet wave
  stage. Waves wash into white and disappear before the logo begins to appear.
  The logo fades in smoothly with a subtle moving gold glint. The portable mark
  uses MINIMALHARD 2's rectangular B and an adapted, legible K; the name uses
  Zrnic. The earlier extra bull and reflected name are removed from this screen.

## Reference and font review

The captured [original PSP sequence](https://www.youtube.com/watch?v=8-_LtPHrMgY)
was inspected in the browser, including its dark light-wave frame and white
logo frame. The owner specified the waves-only stage before the logo.
PS2 game-launch lettering was compared with
[original startup screen examples](https://www.psx-place.com/threads/julians-various-ps2-projects-worklog.20977/page-13).

Fonts used for this rendered artwork:

- Zrnic Regular by Typodermic Fonts, obtained from its
  [author's DaFont listing](https://www.dafont.com/zrnic.font). The included
  desktop license permits rendered audiovisual artwork without font distribution.
- MINIMALHARD 2 by Akihiro OYA, obtained from
  [DaFont](https://www.dafont.com/minimalhard.font), with the included author's
  usage notes. This is reference-based lettering, not an extracted Sony font.
- The existing local Rodin typeface is retained for the company line.

Fonts were loaded only in the local rendering page. No font was installed or
added to the repository or app. These videos remain standalone review previews.

## Checks

All three passed full decode, browser playback, seeking and completion, with
audio/video lengths within one frame. Decoded audio is byte-identical to the
previous revision's verified edited timelines, retaining their zero offset at
10 ms resolution. The PS1 encoded poster has an empty lower footer region.
`verification.json` records these checks. `psp-transition-check.jpg` compares
encoded waves-only, white-transition, logo-reveal and settled frames.

Reproduce using existing FFmpeg/Playwright and the downloaded font paths:

```powershell
$env:KB_WORDMARK_FONT = '<local Zrnic Regular OTF>'
$env:KB_MONOGRAM_FONT = '<local MINIMALHARD 2 TTF>'
$env:KB_INTRO_FONT = '<existing local Rodin TTF>'
node tools/sounds/render-intro-videos.mjs --sony-revision docs/intro-video-previews/sony-revision-3
```

Use a new output directory or `--replace` for these known preview outputs.
