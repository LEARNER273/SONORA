#!/usr/bin/env python3
"""Generate SONORA launcher icons (adaptive foreground + legacy PNGs).

Writes into platform/res/mipmap-*/ — run by scripts/build_apk.sh, or manually:
    python3 scripts/gen_icons.py
"""
import os
from PIL import Image, ImageDraw

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RES = os.path.join(ROOT, "platform", "res")

BAR_HEIGHTS = [0.38, 0.62, 0.95, 0.72, 0.45, 0.28, 0.52]
BAR_COLORS = [
    (34, 211, 238),    # cyan
    (56, 189, 248),
    (125, 211, 252),
    (167, 139, 250),   # violet
    (192, 132, 252),
    (232, 121, 249),
    (244, 114, 182),   # magenta
]


def foreground(size: int) -> Image.Image:
    """Adaptive-icon foreground: transparent canvas, waveform bars inside the
    66/108 adaptive safe zone."""
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    safe = size * 0.66
    n = len(BAR_HEIGHTS)
    bw = safe * 0.11
    gap = safe * 0.055
    total = n * bw + (n - 1) * gap
    x0 = (size - total) / 2
    for i, (h, color) in enumerate(zip(BAR_HEIGHTS, BAR_COLORS)):
        bh = safe * h
        x = x0 + i * (bw + gap)
        y0 = (size - bh) / 2
        draw.rounded_rectangle(
            [x, y0, x + bw, y0 + bh], radius=bw / 2, fill=color + (255,)
        )
    return img


def legacy(size: int) -> Image.Image:
    """Legacy icon: rounded dark square + full-size waveform bars."""
    bg = Image.new("RGBA", (size, size), (11, 15, 30, 255))
    mask = Image.new("L", (size, size), 0)
    md = ImageDraw.Draw(mask)
    md.rounded_rectangle([0, 0, size - 1, size - 1], radius=int(size * 0.22), fill=255)
    icon = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    icon.paste(bg, (0, 0), mask)
    icon.alpha_composite(foreground(size))
    return icon


DENSITIES = {"mdpi": 1.0, "hdpi": 1.5, "xhdpi": 2.0, "xxhdpi": 3.0, "xxxhdpi": 4.0}


def main() -> None:
    for name, scale in DENSITIES.items():
        out_dir = os.path.join(RES, f"mipmap-{name}")
        os.makedirs(out_dir, exist_ok=True)

        fg_size = int(round(108 * scale))
        foreground(fg_size).save(os.path.join(out_dir, "ic_launcher_foreground.png"))

        icon_size = int(round(48 * scale))
        legacy(icon_size).save(os.path.join(out_dir, "ic_launcher.png"))

        print(f"  mipmap-{name}: foreground {fg_size}px, legacy {icon_size}px")
    print("launcher icons written to", RES)


if __name__ == "__main__":
    main()
