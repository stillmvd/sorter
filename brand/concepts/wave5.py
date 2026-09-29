import pathlib
import sys

import wave4

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent / "logo"))
import generate as g

TASKBARS = {"dark": "#202020", "light": "#eeeef0"}


def body(kind):
    base = g.glyph()
    if kind == "circle":
        return f'<circle cx="32" cy="32" r="32" fill="{g.INK}"/>' + g_path(g.fit(base, 64 * .66), g.PAPER)
    if kind == "circle78":
        return f'<circle cx="32" cy="32" r="32" fill="{g.INK}"/>' + g_path(g.fit(base, 64 * .78), g.PAPER)
    if kind == "ring":
        return (f'<circle cx="32" cy="32" r="31" fill="{g.INK}" stroke="#5a5a62" stroke-width="2"/>'
                + g_path(g.fit(base, 64 * .78), g.PAPER))
    if kind == "square":
        return (f'<rect x="1" y="1" width="62" height="62" rx="15" fill="{g.INK}" stroke="#5a5a62" stroke-width="2"/>'
                + g_path(g.fit(base, 64 * .8), g.PAPER))
    if kind in ("flip", "flipsq"):
        return ""
    return g_path(g.fit(base, 60), g.PAPER, outline=g.INK)


def flipped(kind, bar):
    base = g.glyph()
    plate, ink = (g.PAPER, g.INK) if bar == "dark" else (g.INK, g.PAPER)
    shape = (f'<circle cx="32" cy="32" r="32" fill="{plate}"/>' if kind == "flip"
             else f'<rect width="64" height="64" rx="15" fill="{plate}"/>')
    return shape + g_path(g.fit(base, 64 * (.78 if kind == "flip" else .8)), ink)


def g_path(geom, fill, outline=None):
    stroke = f' stroke="{outline}" stroke-width="3" paint-order="stroke" stroke-linejoin="round"' if outline else ""
    return f'<path d="{g.path_d(geom)}" fill="{fill}" fill-rule="evenodd"{stroke}/>'


def overflow(kind, bar):
    from shapely import affinity
    from shapely.geometry import Polygon
    from shapely.ops import unary_union
    cx, cy, r, size, gx, gy = (38, 26, 24, 56, 29, 36) if kind == "offset" else (32, 32, 29, 58, 32, 32)
    geom = g.fit(g.glyph(), size)
    x0, y0, x1, y1 = geom.bounds
    geom = affinity.translate(geom, gx - (x0 + x1) / 2, gy - (y0 + y1) / 2)
    silhouette = unary_union([Polygon(p.exterior) for p in getattr(geom, "geoms", [geom])])
    plate = g.PAPER if bar == "dark" else g.INK
    return (f'<circle cx="{cx}" cy="{cy}" r="{r}" fill="{plate}"/>'
            f'<path d="{g.path_d(silhouette)}" fill="{g.INK}" stroke="{g.INK}" stroke-width="3.5" stroke-linejoin="round"/>'
            f'<path d="{g.path_d(geom)}" fill="{g.PAPER}" fill-rule="evenodd"/>')


def variant_svg(cid, key, in_circle):
    if not key.startswith("t-"):
        return wave4.variant_svg(cid, key, in_circle)
    _, kind, bar = key.split("-")
    if kind in ("offset", "center"):
        inner = overflow(kind, bar)
    elif kind == "flip":
        inner = flipped(kind, bar)
    else:
        inner = body(kind)
    return inner


def surface(key):
    return f" bar-{key.rsplit('-', 1)[1]}" if key.startswith("t-") else ""


NUMBER = 5
TITLE = "Волна 5 · иконка на панели задач"
PROMPT = ("На панели задач знак 24–36 px, и тёмный круг на тёмной панели пропадает. Для 48 px и меньше нужен свой "
          "вариант. Плитки — на цвете панели задач Windows. Отметь один.")
CHOSEN = ["D светлая панель", "три дырочки по бокам"]
KINDS = [("circle", "Как сейчас", "Тёмный круг, глиф 66 %. Для сравнения — это то, что ты видишь на панели."),
         ("flip", "Круг по теме", "Глиф 78 %: на тёмной панели светлый круг, на светлой — тёмный. "
                                  "Работает для запущенного окна; exe и ярлык — светлый круг."),
         ("offset", "Колода перед кругом", "Карты на почти всё поле, с тёмной обводкой; круг сдвинут вверх-вправо "
                                           "и выглядывает из-за колоды. Круг по теме: на тёмной панели светлый."),
         ("center", "Колода шире круга", "Круг по центру, карты крупнее и выходят за его край. Круг по теме.")]
CARDS = [("Мелкие размеры", [
    {"code": chr(65 + i), "name": name, "note": note,
     "tiles": [(f"{chr(65 + i)} тёмная панель", "тёмная панель", f"t-{kind}-dark", False),
               (f"{chr(65 + i)} светлая панель", "светлая панель", f"t-{kind}-light", False)]}
    for i, (kind, name, note) in enumerate(KINDS)])]
SUMMARY = ("Моё мнение: C «Колода перед кругом» — знак почти вдвое крупнее, чем сейчас, круг виден и работает как фон-"
           "пятно, тёмная обводка держит карты на светлом. Цвет круга меняется по теме панели у запущенного окна; "
           "exe и ярлык — светлый круг. Размеры от 64 px и знак в интерфейсе не меняются.")
