"""Renders Nitrate's own demo footage for screenshots: a sunset over layered ridges.

Everything is drawn here (numpy), so the docs and the website carry no third-party media.
Writes 12 s of 1080p30 video, then loops it losslessly to a 10-minute file so the
timeline in screenshots looks like a real film:

    python scripts/demo-footage.py <out-dir>      # writes "<out-dir>/Ember Ridge.mp4"

Needs numpy and ffmpeg on PATH. Encoding uses at most 4 threads.
"""

import os
import subprocess
import sys

import numpy as np

W, H, FPS, SECONDS = 1920, 1080, 30, 12
TITLE = "Ember Ridge"


def ridge(rng, octaves, base, amp, width):
    """A tileable 1-D height profile: sum of sines with random phases."""
    x = np.arange(width) / width * 2 * np.pi
    y = np.zeros(width)
    for o in range(octaves):
        f = 2 ** o * rng.integers(1, 3)
        y += amp / (1.6 ** o) * np.sin(f * x + rng.uniform(0, 2 * np.pi))
    return base + y


def mix(a, b, t):
    t = np.clip(t, 0, 1)[..., None]
    return a * (1 - t) + b * t


def main():
    out_dir = sys.argv[1] if len(sys.argv) > 1 else "."
    os.makedirs(out_dir, exist_ok=True)
    clip = os.path.join(out_dir, "ember-ridge-clip.mp4")
    final = os.path.join(out_dir, f"{TITLE}.mp4")
    rng = np.random.default_rng(7)

    yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
    v = yy / H
    # Sky: deep violet at the top, ember orange at the horizon.
    top = np.array([24, 16, 46], np.float32)
    mid = np.array([150, 52, 70], np.float32)
    low = np.array([255, 150, 70], np.float32)
    sky = mix(mix(top, mid, v / 0.45), low, (v - 0.45) / 0.25)

    # Five ridges, far (hazy, warm) to near (dark), each wider than the frame for parallax.
    layers = []
    for i in range(5):
        width = W * 3
        base = H * (0.58 + 0.08 * i)
        prof = ridge(rng, 6, base, H * (0.05 + 0.02 * i), width)
        haze = 1 - i / 4
        colour = mix(np.array([18, 10, 22], np.float32), np.array([200, 90, 80], np.float32), np.array(haze * 0.75))
        layers.append((prof, colour.reshape(3), 6 + 22 * i))

    sun_x, sun_r = W * 0.62, H * 0.075
    frames = FPS * SECONDS
    enc = subprocess.Popen(
        [
            "ffmpeg", "-y", "-loglevel", "error",
            "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", f"{W}x{H}", "-r", str(FPS), "-i", "-",
            "-f", "lavfi", "-t", str(SECONDS), "-i", "anullsrc=r=48000:cl=stereo",
            "-c:v", "libx264", "-preset", "medium", "-crf", "18", "-pix_fmt", "yuv420p", "-threads", "4",
            "-c:a", "aac", "-shortest", clip,
        ],
        stdin=subprocess.PIPE,
    )
    for n in range(frames):
        t = n / frames
        img = sky.copy()
        # The sun sinks slowly; a soft glow around it.
        sy = H * (0.50 + 0.06 * t)
        d = np.hypot(xx - sun_x, yy - sy)
        glow = np.exp(-((d / (H * 0.35)) ** 2))[..., None]
        img = img + glow * np.array([90, 45, 10], np.float32)
        img = mix(img, np.array([255, 236, 190], np.float32), (sun_r - d) / 3)
        # Drifting cloud bands.
        band = np.sin(yy / 23 + np.sin(xx / 310 + t * 6.28) * 2) * np.sin(xx / 170 - t * 3.0)
        cloud = np.clip(band, 0, 1) * np.exp(-((v - 0.3) / 0.09) ** 2)
        img = mix(img, np.array([255, 170, 120], np.float32), cloud * 0.35)
        for prof, colour, speed in layers:
            shift = int(t * speed * 4) % prof.size
            line = np.roll(prof, -shift)[:W]
            mask = (yy >= line[None, :]).astype(np.float32)
            img = mix(img, colour, mask)
        # Vignette and a little film grain.
        vig = 1 - 0.35 * (((xx - W / 2) / W) ** 2 + ((yy - H / 2) / H) ** 2) * 2
        img = img * vig[..., None] + rng.normal(0, 2.5, (H, W, 1))
        enc.stdin.write(np.clip(img, 0, 255).astype(np.uint8).tobytes())
    enc.stdin.close()
    if enc.wait() != 0:
        sys.exit("ffmpeg failed")
    # 50 loops of 12 s: a 10-minute file without re-encoding.
    subprocess.run(
        [
            "ffmpeg", "-y", "-loglevel", "error", "-stream_loop", "49", "-i", clip,
            "-c", "copy", "-metadata", f"title={TITLE}", final,
        ],
        check=True,
    )
    os.remove(clip)
    print(final)


if __name__ == "__main__":
    main()
