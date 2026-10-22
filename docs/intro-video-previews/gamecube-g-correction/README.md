# GameCube G correction

Corrects the mirrored right face in revision 3. The trail now runs across the
bottom of that face, rises at its outer edge, and turns inward along the top.
This closes the lower outline of the G while retaining the opening above its
crossbar. Purple styling, KB branding and the full supplied audio are retained.

The final shape was visually compared with this
[GameCube logo reference](https://www.pngkit.com/view/u2q8w7r5i1w7a9q8_gamecube-logo-png-nintendo-gamecube-logo/).
The MP4 passed full decode and stream/duration checks. Audio has zero offset at
10 ms envelope resolution and correlation above 0.99999 with the original.
Details are in `verification.json`; the PNG is from the encoded movie.

Render only this preview:

```powershell
node tools/sounds/render-intro-videos.mjs --classic --only=gamecube docs/intro-video-previews/gamecube-g-correction
```
