# Console-style KB intro revisions

These are the earlier previews. [Revision 3](../revision-3/README.md) contains
the requested GameCube geometry, Game Boy shine and PS3 lettering corrections.

Requested 2026-10-22: make the three videos look like the original console
startups, with KB's name and logo. These replace the creative direction of the
Prism/Orbit/Aurora previews; earlier previews remain available for comparison.

| Preview | Direction |
|---|---|
| `kb-gba-classic.mp4` | White screen, rainbow letters flying in from the lower right, heavy blue italic wordmark, two-tier naming, small KB bull at the cartridge-logo position |
| `kb-gamecube-classic.mp4` | Pure black screen, solid lavender cube tumbling on the chimes and tracing an isometric outline, central KB bull and white wordmark |
| `kb-ps3-classic.mp4` | Later PS3-style warm dark XMB background, broad translucent wave, restrained right-side white KB monogram/bull and KaneegaBell subtitle |

The original KB bull artwork is adapted to monochrome with its broken horn,
eyes, nostrils and nose-ring opening preserved. The app-icon tile is removed.
No console wordmarks, extracted firmware assets or proprietary fonts are used.
These are recreations of the visual sequences with KB branding, not original
console footage with a logo pasted over it.

Reference review:

- [GBA startup comparison recording](https://www.youtube.com/watch?v=nlBZE_BAuXY)
- [GameCube launch-day screen reference](https://game.watch.impress.co.jp/docs/20010914/gc_1st.htm)
- [Sony's startup-sequence retrospective and PS3 reference image](https://blog.playstation.com/2019/12/05/how-takafumi-fujisawa-created-the-original-playstations-startup-sound/)

The same complete supplied GBA, GameCube and PS3 audio is embedded in these
1920×1080 / 60 fps H.264 + AAC MP4 previews. Source recordings are unchanged.
Encoding checks include full decode, duration/stream validation and A/V length
within one frame; results are in `verification.json`. Posters and a contact
sheet show frames taken from the actual encoded movies.

Render: `node tools/sounds/render-intro-videos.mjs --classic`.
Use a new output directory or explicitly add `--replace` to regenerate these
known preview files. The drawing source is `tools/sounds/classic-intro-scenes.js`.
No app integration, developer build, installer change or publication was made.
