# Remaining console-style KB previews

Created 2026-10-22 following the request to make the other intros as similar to
their originals as possible, using the supplied recordings and KB branding.
These are standalone review videos; app intro assets have not been replaced.

| Video | Visual sequence | Full recording |
|---|---|---:|
| `kb-ps1-classic.mp4` | Gray screen, navy serif name and warm diamond with KB bull; black second screen with four-color bull and licensing layout | 21.891 s |
| `kb-ps2-classic.mp4` | Moving camera through translucent towers and blue haze; white company line; black transition and blue geometric KB title | 16.632 s |
| `kb-psp-classic.mp4` | Dark blue wave and company line, then white game-launch screen with thin KB lettering and reflection | 6.165 s |
| `kb-xbox360-classic.mp4` | Green orbiting ribbons, lit metallic sphere, expanding ring and pale background with green KB naming | 7.863 s |
| `kb-switch-classic.mp4` | White controller halves on red; right half snaps down at the supplied 60 ms click | 0.828 s |
| `kb-konami-classic.mp4` | Metallic folded ribbons assemble, turn orange/red and resolve over white with italic KB naming; both recorded phrases retained | 10.446 s |
| `kb-rockstar-classic.mp4` | Two engraved metallic shell heads, closing action, flash and drifting smoke on the shotgun blast, then red KB bull on black | 11.077 s |

The drawings follow the original visual motifs and sequence structure with new
KB branding. They are recreations, not captures of original console footage.
Custom geometric lettering approximates the PS2/PSP wordmarks. The locally
available Rodin font was loaded temporarily for the company lines; no typeface
file was installed or added to the repository/app.

References reviewed include the
[PlayStation startup retrospective](https://blog.playstation.com/archive/2019/12/05/how-sound-designer-takafumi-fujisawa-created-the-original-playstations-iconic-startup-sound),
[PS1 frame comparison](https://forums.libretro.com/t/graphics-problems-in-ps1-game-beetle-psx-hw/29492),
[PS2 startup towers](https://www.gamesradar.com/ps2s-startup-screen-easter-egg-is-surprising-players-all-over-again/),
[PSP startup reference](https://makeagif.com/amp/1HnVLQ),
[Xbox startup sequence description](https://comicbook.com/gaming/news/xbox-startup-screens-new-video/),
[Konami animation description](https://closinglogogroup.fandom.com/wiki/Konami/Summary),
and [RDR2 shell framing](https://www.gta5-mods.com/misc/red-dead-redemption-2-startup-logo-video).

Every video is 1920×1080 / 60 fps H.264 with AAC stereo audio. Both original PSP
parts were concatenated in order into a temporary lossless WAV for encoding,
then the temporary WAV was removed. All other videos use their supplied MP3s
directly. The originals are unchanged and each full recording is preserved.

Validation: full movie decode; browser playback, seeking and completion; stream
dimensions/rate/codecs; audio/video lengths within one frame. Audio-envelope
comparison with the supplied recordings found zero offset at 10 ms resolution
for all seven, with correlation above 0.9998. See `verification.json`.
The PNG posters and `contact-sheet.jpg` come from the encoded movies.

Render using the existing Node, FFmpeg and Playwright installations:

```powershell
# Optionally set KB_INTRO_FONT to the existing local Rodin TTF for company lines.
node tools/sounds/render-intro-videos.mjs --remaining docs/intro-video-previews/remaining-classic
```

Use a new output folder or explicitly add `--replace` for these known previews.
`--only=ps1` (or another listed key) renders one video. Source drawings are in
`tools/sounds/classic-intro-scenes.js`. No installer or release was produced.
