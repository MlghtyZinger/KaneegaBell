# GameCube landing bounce

Retains the accepted G geometry and adds two diminishing vertical rebounds to
the center cube after its 5.23-second landing. The first rebound lasts 0.32 s;
the smaller second lasts 0.20 s. The cube is fully settled by 5.75 s.

The complete supplied recording remains on the same timeline. The 1080p/60 fps
MP4 passed full decode and audio/video duration checks; see `verification.json`.
`bounce-check.jpg` compares the encoded landing, rebound and settled frames.

Render with `node tools/sounds/render-intro-videos.mjs --classic --only=gamecube docs/intro-video-previews/gamecube-bounce`.
