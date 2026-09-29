import io
import math
import pathlib
import subprocess

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from PIL import Image
from shapely import affinity
from shapely.geometry import Polygon, box
from shapely.ops import unary_union

ROOT = pathlib.Path(__file__).resolve().parent
PROJECT = ROOT.parent.parent
ICONS = PROJECT / "src-tauri/icons"
INKSCAPE = "C:/Program Files/Inkscape/bin/inkscape.com"
FONT = PROJECT / "src/fonts/KockersSans-Bold.woff2"

INK = "#17171a"
PAPER = "#f4f4f6"
DARK = "#141416"
W, H, R, D, GAP = 24, 30, 3.5, 5.5, 2.2
HOLES, PLAY = 4, 9
FRACTION = .66
ICO_SIZES = [24, 30, 32, 36, 40, 48, 60, 64, 72, 96, 128, 256]


def rrect(x, y, w, h, r):
    return box(x + r, y + r, x + w - r, y + h - r).buffer(r, quad_segs=16)


def play(cx, cy, h):
    w = h * math.sqrt(3) / 2
    x0 = cx - w / 3
    return Polygon([(x0, cy - h / 2), (x0 + w, cy), (x0, cy + h / 2)])


def glyph():
    m = 3
    step = (H - 2 * m) / HOLES
    hh, hw = step * .55, 3
    holes = [rrect(side, m + step * k + (step - hh) / 2, hw, hh, .8)
             for side in (1.8, W - 1.8 - hw) for k in range(HOLES)]
    front = rrect(0, 0, W, H, R).difference(unary_union(holes + [play(W / 2, H / 2, PLAY)]))
    mid = rrect(D, -D, W, H, R)
    back = rrect(2 * D, -2 * D, W, H, R)
    mid_visible = mid.difference(rrect(0, 0, W, H, R).buffer(GAP, quad_segs=16))
    back_visible = back.difference(mid.buffer(GAP, quad_segs=16))
    return unary_union([front, mid_visible, back_visible])


def fit(geom, size, center=32):
    x0, y0, x1, y1 = geom.bounds
    s = size / max(x1 - x0, y1 - y0)
    geom = affinity.scale(geom, s, s, origin=(0, 0))
    x0, y0, x1, y1 = geom.bounds
    return affinity.translate(geom, center - (x0 + x1) / 2, center - (y0 + y1) / 2)


def ring_d(coords):
    pts = list(coords)[:-1]
    return "M" + "L".join(f"{x:.2f} {y:.2f}" for x, y in pts) + "Z"


def path_d(geom):
    polys = getattr(geom, "geoms", [geom])
    return "".join(ring_d(p.exterior.coords) + "".join(ring_d(i.coords) for i in p.interiors) for p in polys)


def svg(inner, view="0 0 64 64", size=None):
    dims = f' width="{size}" height="{size}"' if size else ""
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{view}"{dims}>{inner}</svg>\n'


def wordmark(height):
    font = TTFont(io.BytesIO(FONT.read_bytes()))
    cmap, glyphs = font.getBestCmap(), font.getGlyphSet()
    upm = font["head"].unitsPerEm
    cap = font["OS/2"].sCapHeight or upm * .7
    s = height / cap
    pen = SVGPathPen(glyphs)
    x = 0
    for ch in "Sorter":
        name = cmap[ord(ch)]
        glyphs[name].draw(TransformPen(pen, (s, 0, 0, -s, x * s, height)))
        x += glyphs[name].width
    return pen.getCommands(), x * s


def main():
    base = glyph()
    mono = path_d(fit(base, 64))
    circle = f'<circle cx="32" cy="32" r="32" fill="{INK}"/><path d="{path_d(fit(base, 64 * FRACTION))}" fill="{PAPER}" fill-rule="evenodd"/>'
    (ROOT / "mark.svg").write_text(svg(circle), encoding="utf-8")
    (ROOT / "app-icon.svg").write_text(svg(circle, size=256), encoding="utf-8")
    (ROOT / "mark-mono.svg").write_text(svg(f'<path d="{mono}" fill="currentColor" fill-rule="evenodd"/>'),
                                        encoding="utf-8")
    words, words_w = wordmark(36)
    gap = 22
    total = 64 + gap + words_w
    for name, color in (("lockup.svg", INK), ("lockup-on-dark.svg", PAPER)):
        inner = (f'<path d="{mono}" fill="{color}" fill-rule="evenodd"/>'
                 f'<path d="{words}" fill="{color}" transform="translate({64 + gap} 14)"/>')
        (ROOT / name).write_text(svg(inner, view=f"0 0 {total:.1f} 64"), encoding="utf-8")
    (PROJECT / "src/components/ui/markPath.ts").write_text(f'export const MARK_PATH = "{mono}";\n', encoding="utf-8")

    tmp = ROOT / "png"
    tmp.mkdir(exist_ok=True)
    for px in sorted(set(ICO_SIZES + [32, 128, 256])):
        subprocess.run([INKSCAPE, str(ROOT / "mark.svg"),
                        f"--actions=export-filename:{tmp / f'{px}.png'};export-width:{px};export-height:{px};export-do"],
                       check=True, capture_output=True)
    for name, px in (("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256)):
        (ICONS / name).write_bytes((tmp / f"{px}.png").read_bytes())
    images = [Image.open(tmp / f"{px}.png").convert("RGBA") for px in ICO_SIZES]
    images[-1].save(ICONS / "icon.ico", format="ICO", sizes=[(px, px) for px in ICO_SIZES],
                    append_images=images[:-1])
    print("ok", ROOT)


if __name__ == "__main__":
    main()
