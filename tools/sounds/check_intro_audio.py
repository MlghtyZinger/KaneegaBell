"""Measure bundled intro recordings without modifying the owner's originals.

Usage: python tools/sounds/check_intro_audio.py <output-directory>
Requires the existing ffmpeg and numpy; writes an envelope report and waveform PNG.
"""
import json
from pathlib import Path
import subprocess
import sys

import numpy as np
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parents[2]
RATE = 16000
WINDOW = 160  # 10 ms, after decoding including the recording's silence.


def envelope(path):
    raw = subprocess.check_output([
        "ffmpeg", "-v", "error", "-i", str(path), "-ac", "1", "-ar", str(RATE),
        "-f", "f32le", "-",
    ])
    pcm = np.frombuffer(raw, dtype="<f4")
    bins = pcm[:len(pcm) // WINDOW * WINDOW].reshape(-1, WINDOW)
    rms = np.sqrt(np.mean(bins ** 2, axis=1))
    return len(pcm) / RATE, rms


def main():
    out = Path(sys.argv[1])
    out.mkdir(parents=True, exist_ok=True)
    files = sorted((ROOT / "src/assets/sounds").glob("intro_*.ogg"))
    image = Image.new("RGB", (1500, 130 * len(files)), "white")
    draw = ImageDraw.Draw(image)
    results = []
    for row, path in enumerate(files):
        duration, rms = envelope(path)
        active = np.flatnonzero(rms > max(float(rms.max()) * .01, .0005))
        # The strongest energy rises, spaced apart so sustained ringing is not
        # mistaken for many distinct cues. The report still needs human review.
        rise = rms - np.r_[np.zeros(5), rms[:-5]]
        chosen = []
        for index in np.argsort(rise)[::-1]:
            t = float(index / 100)
            if rise[index] <= rms.max() * .025:
                break
            if all(abs(t - old) > .28 for old in chosen):
                chosen.append(t)
            if len(chosen) == 12:
                break
        result = dict(key=path.stem.removeprefix("intro_"), duration=round(duration, 4),
                      firstSound=round(float(active[0] / 100), 3),
                      lastSound=round(float(active[-1] / 100), 3),
                      strongestRises=sorted(chosen))
        results.append(result)
        y = row * 130
        draw.text((10, y + 8), json.dumps(result), fill="black")
        scale = 1300 / duration
        points = [(170 + i / 100 * scale, y + 110 - float(v / rms.max()) * 75) for i, v in enumerate(rms)]
        draw.line(points, fill="#2868a0", width=1)
        for t in chosen:
            x = 170 + t * scale
            draw.line((x, y + 30, x, y + 112), fill="#d9b38c")
            draw.text((x, y + 113), str(round(t, 2)), fill="black")
    (out / "intro-envelopes.json").write_text(json.dumps(results, indent=2) + "\n", encoding="utf-8")
    image.save(out / "intro-envelopes.png")
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
