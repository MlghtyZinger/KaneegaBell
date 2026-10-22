# PS1, PS2 and PSP corrections

2026-10-22. The owner requested closer reference matching and explicitly allowed
audio cuts to avoid long intros and pauses. These standalone review files
supersede the first three videos in `remaining-classic`; originals stay intact.

- PS1 — 7.8 seconds. Gray first screen, navy text and a red-to-gold diamond;
  the second screen uses a red upright bull over a yellow/teal/blue KB base,
  matching the original logo's color placement rather than coloring the bull's
  face purple and lime. Licensing text is laid out closer to the reference.
- PS2 — 7.6 seconds. Towers fade directly into the blue KB title on the final
  cue. The measured interior black transition is 0.20 seconds, replacing the
  earlier multi-second wait.
- PSP — 6.165 seconds. Violet and blue translucent waves sweep through the
  game-launch transition. A warm gold flare crosses the KB logo as it appears;
  both fade away into the white launch screen. Both PSP recordings are retained.

PS1 color/placement reference:
[original startup frame](https://www.everyeye.it/notizie/playstation-1-modder-scopre-dettaglio-sorprendente-schermata-iniziale-527381.html).
PSP transition reference:
[PSP startup animation](https://makeagif.com/gif/psp-startup-screen-peONjP),
along with the owner's specific purple/blue wave and yellow flare direction.
These remain KB-branded recreations, with geometric lettering approximations.

## Audio edits

The source MP3s were only read. Edits were made to temporary WAV copies, deleted
after encoding. No pitch or speed changes were made.

| Preview | Source ranges, seconds | Result |
|---|---|---:|
| PS1 | 0–4.3, then 12.7–16.2 | 7.8 s |
| PS2 | 0–5.6, then 14.4–16.4 | 7.6 s |
| PSP | Complete part 1 followed by complete part 2 | 6.165 s |

Edited segment boundaries have an 8 ms fade-in and 25 ms fade-out to avoid
clicks. The PS2 final sound at source 14.63 s now lands at output 5.83 s, aligned
with the title reveal. The scene timings follow these edited timelines.

Future intro previews should avoid long silent holds and use short audio edits
when needed. The owner's previously accepted GameCube preview is preserved.

All three are 1080p/60 fps H.264 + AAC. Full decode, browser playback, seeking
and completion passed. Audio/video lengths agree within one frame. Comparison
against the edited audio timelines found zero offset at 10 ms resolution,
with audio-envelope correlation above 0.9999. See `verification.json`.
The posters and transition PNGs were taken from the encoded videos.

Render with the existing local tools:

```powershell
node tools/sounds/render-intro-videos.mjs --sony-revision docs/intro-video-previews/sony-revision-2
```

Set `KB_INTRO_FONT` to the existing local Rodin TTF to reproduce company-line
lettering. The font is loaded only in the rendering page and is not bundled.
Use a new output directory or `--replace` for these known preview files.
