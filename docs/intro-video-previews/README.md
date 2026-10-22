# Three new KB intro video previews

Latest versions: [Sony revision 5](sony-revision-5/README.md). PS1 is 10 seconds,
PS2 is 9.6 seconds, and PSP shows KB over Kaneegabell Computer Entertainment.

Current PS1/PS2/PSP previews: [Sony revision 4](sony-revision-4/README.md), using
the established KB bull on PS1, a full-name PSP wordmark, and reveal timing
checked against the actual audio attacks.

Latest PS1/PS2/PSP previews: [Sony revision 3](sony-revision-3/README.md), with a
new PS1 final mark and no bottom KB, corrected wordmark fonts, and a separate
PSP waves-only stage before the smooth logo reveal.

Newest PS1/PS2/PSP versions: [shorter Sony revisions](sony-revision-2/README.md),
with corrected PS1 colors, the PS2 wait removed, and PSP purple/blue waves plus
a gold flare. PS1 and PS2 use edited audio timelines under eight seconds.

New previews for the seven other intros: [remaining console-style previews](remaining-classic/README.md)
(PS1, PS2, PSP, Xbox 360, Switch, Konami and Rockstar).

Latest corrections: [revision 3](revision-3/README.md), including the cubic
GameCube G, mixed-case Game Boy name and final-chime shine, and PS3 text/typeface.
The [subsequent GameCube correction](gamecube-g-correction/README.md) fixes the
mirrored right face so the final outline reads as G.
The [latest GameCube preview](gamecube-bounce/README.md) retains that accepted
shape and adds the requested final center-cube bounce.

Created 2026-10-22 using the owner's existing full recordings and KB's original
bull artwork. These are newly drawn scenes, not captures of the current CSS
intros. They are review previews and are not yet installed as app intros.

| Video | Recording | Duration | Visual direction |
|---|---|---:|---|
| `kb-gba-prism.mp4` | `game-boy-advance-sp-intro.mp3` | 4.76 s | Light glass sheets assemble on the opening notes; final-chime orbit and glint |
| `kb-gamecube-orbit.mp4` | `gamecube intro.mp3` | 10.124 s | Glass cubes arrive on the chimes, converge and resolve into KB at 5.23 s |
| `kb-ps3-aurora.mp4` | `ps3boot.mp3` | 6.687 s | Luminous blue ribbons, particles and a central reveal at the crescendo |

All are 1920×1080, 60 fps, H.264 video with AAC stereo audio and fast-start MP4
layout. The full audio is baked into the same playback timeline as the visuals.
Original recordings, current intro assets, installed app and both libraries are
unchanged. No dependencies were installed and no release was published.

Checks: full movie decode, stream dimensions/rate/codecs, complete recording
duration, A/V duration within one frame, and source/export audio-envelope
correlation. At 10 ms envelope resolution, all three exports measured zero
audio offset; correlation exceeded 0.99999. Encoded keyframes were visually
inspected. See `verification.json`, the individual PNG posters and
`contact-sheet.jpg`.

Render using existing FFmpeg/Playwright with:

```powershell
node "C:\Emulator files\tools\sounds\render-intro-videos.mjs" "C:\Emulator files\docs\another-preview-folder"
```

The renderer refuses to overwrite existing movie previews. Drawings are in
`tools/sounds/intro-video-scenes.js`; timings are explicit and deterministic.
The videos use the supplied Nintendo/Sony recordings; the new visuals do not
change the recordings' ownership or grant a new audio license.

A fixed movie avoids separate animation/audio clocks. The tradeoffs for the app
would be larger assets and losing automatic compatibility with arbitrary custom
audio overrides. Those decisions can follow after the owner reviews the videos.
