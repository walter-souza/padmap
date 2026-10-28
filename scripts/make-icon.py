"""Gera assets/padmap.ico e assets/padmap.png a partir das formas de
assets/padmap.svg (repetidas aqui, porque são só retângulos e um círculo).

Uso: python scripts/make-icon.py  (precisa do Pillow)
"""

from pathlib import Path

from PIL import Image, ImageDraw

ASSETS = Path(__file__).resolve().parent.parent / "assets"
SIZES = [16, 24, 32, 48, 64, 128, 256]
# Desenha 8 vezes maior e reduz, para as bordas ficarem suaves.
SUPER = 8

# Mesmas formas do SVG, no espaço 64 × 64: (x, y, largura, altura, raio, cor).
RECTS = [
    (4, 6, 56, 54, 13, "#141A28"),
    (7, 5, 50, 47, 11, "#2E3547"),
    (26, 13, 12, 32, 3, "#3B82F6"),
    (16, 23, 32, 12, 3, "#3B82F6"),
]
CIRCLE = (32, 29, 3, "#1D4ED8")


def render(size: int) -> Image.Image:
    big = size * SUPER
    k = big / 64
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    for x, y, w, h, r, color in RECTS:
        d.rounded_rectangle(
            (x * k, y * k, (x + w) * k - 1, (y + h) * k - 1), radius=r * k, fill=color
        )
    cx, cy, r, color = CIRCLE
    d.ellipse(((cx - r) * k, (cy - r) * k, (cx + r) * k, (cy + r) * k), fill=color)
    return img.resize((size, size), Image.LANCZOS)


def main() -> None:
    images = [render(s) for s in SIZES]
    images[-1].save(ASSETS / "padmap.png")
    images[-1].save(ASSETS / "padmap.ico", sizes=[(s, s) for s in SIZES], append_images=images[:-1])


if __name__ == "__main__":
    main()
