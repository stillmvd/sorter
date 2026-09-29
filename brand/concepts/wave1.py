import math

INK = "#17171a"
PAPER = "#f4f4f6"


def rot(pt, c, deg):
    a = math.radians(deg)
    x, y = pt[0] - c[0], pt[1] - c[1]
    return (c[0] + x * math.cos(a) - y * math.sin(a), c[1] + x * math.sin(a) + y * math.cos(a))


def rrect_d(x, y, w, h, r):
    return (f"M{x + r:.2f} {y:.2f}H{x + w - r:.2f}A{r} {r} 0 0 1 {x + w:.2f} {y + r:.2f}V{y + h - r:.2f}"
            f"A{r} {r} 0 0 1 {x + w - r:.2f} {y + h:.2f}H{x + r:.2f}A{r} {r} 0 0 1 {x:.2f} {y + h - r:.2f}"
            f"V{y + r:.2f}A{r} {r} 0 0 1 {x + r:.2f} {y:.2f}Z")


def play_d(cx, cy, h):
    w = h * math.sqrt(3) / 2
    x0 = cx - w / 3
    return f"M{x0:.2f} {cy - h / 2:.2f}L{x0 + w:.2f} {cy:.2f}L{x0:.2f} {cy + h / 2:.2f}Z"


def fill(d, pts, angle=0, pivot=(0, 0)):
    return {"kind": "fill", "d": d, "pts": pts, "angle": angle, "pivot": pivot}


def card(x, y, w, h, r, holes="", gap=0, angle=0, pivot=(0, 0)):
    pts = [(x, y), (x + w, y), (x, y + h), (x + w, y + h)]
    return {"kind": "card", "rect": (x, y, w, h, r), "holes": holes, "gap": gap, "pts": pts,
            "angle": angle, "pivot": pivot}


def line(pts, w):
    return {"kind": "line", "pts": pts, "w": w, "angle": 0, "pivot": (0, 0)}


def arc(cx, cy, r, a0, a1, step=3):
    n = max(2, int(abs(a1 - a0) / step) + 1)
    return [(cx + r * math.cos(math.radians(a0 + (a1 - a0) * i / (n - 1))),
             cy + r * math.sin(math.radians(a0 + (a1 - a0) * i / (n - 1)))) for i in range(n)]


def bbox(items):
    xs, ys = [], []
    for it in items:
        pad = it.get("w", 0) / 2
        for p in it["pts"]:
            x, y = rot(p, it["pivot"], it["angle"])
            xs += [x - pad, x + pad]
            ys += [y - pad, y + pad]
    return min(xs), min(ys), max(xs), max(ys)


def render(items, fg, bg):
    out = []
    for it in items:
        tr = f' transform="rotate({it["angle"]} {it["pivot"][0]} {it["pivot"][1]})"' if it["angle"] else ""
        if it["kind"] == "fill":
            out.append(f'<path d="{it["d"]}" fill="{fg}" fill-rule="evenodd"{tr}/>')
        elif it["kind"] == "line":
            d = "M" + " L".join(f"{x:.2f} {y:.2f}" for x, y in it["pts"])
            out.append(f'<path d="{d}" fill="none" stroke="{fg}" stroke-width="{it["w"]}" '
                       f'stroke-linecap="round" stroke-linejoin="round"/>')
        else:
            x, y, w, h, r = it["rect"]
            g = it["gap"]
            if g:
                out.append(f'<path d="{rrect_d(x - g, y - g, w + 2 * g, h + 2 * g, r + g)}" fill="{bg}"{tr}/>')
            out.append(f'<path d="{rrect_d(x, y, w, h, r)}{it["holes"]}" fill="{fg}" fill-rule="evenodd"{tr}/>')
    return "".join(out)


def glyph_svg(items, size, fg, bg):
    x0, y0, x1, y1 = bbox(items)
    s = size / max(x1 - x0, y1 - y0)
    tx = 32 - s * (x0 + x1) / 2
    ty = 32 - s * (y0 + y1) / 2
    return f'<g transform="translate({tx:.3f} {ty:.3f}) scale({s:.4f})">{render(items, fg, bg)}</g>'


def perfs(x, y, w, n, hw, hh):
    step = w / n
    return "".join(rrect_d(x + step * (i + .5) - hw / 2, y, hw, hh, min(hw, hh) / 3) for i in range(n))


def frame_to_pile():
    w, h = 32, 24
    holes = perfs(0, 2.2, w, 4, 3.6, 3) + perfs(0, h - 5.2, w, 4, 3.6, 3) + play_d(w / 2, h / 2, 9)
    return [card(0, 0, w, h, 3.5, holes),
            fill(rrect_d(2, h + 3.5, w - 4, 4, 2), [(2, h + 3.5), (w - 2, h + 7.5)]),
            fill(rrect_d(5, h + 11, w - 10, 4, 2), [(5, h + 11), (w - 5, h + 15)])]


def strip_pulled():
    w, h, g, shift = 22, 15, 3.2, 9
    items = []
    for i in range(3):
        x = shift if i == 1 else 0
        y = i * (h + g)
        holes = "".join(rrect_d(x + side, y + 2.4 + k * 4.2, 2.6, 2.4, .8)
                        for side in (1.6, w - 4.2) for k in range(3))
        holes += play_d(x + w / 2, y + h / 2, 6.5) if i == 1 else ""
        items.append(card(x, y, w, h, 2.6, holes))
    return items


def fan():
    w, h, pivot = 22, 30, (11, 42)
    front = card(0, 0, w, h, 3.5, play_d(w / 2, h / 2, 11), gap=2.4)
    return [card(0, 0, w, h, 3.5, angle=-20, pivot=pivot),
            card(0, 0, w, h, 3.5, angle=20, pivot=pivot),
            front]


def offset_stack():
    w, h, dx, dy = 24, 30, 5.5, -5.5
    return [card(2 * dx, 2 * dy, w, h, 3.5),
            card(dx, dy, w, h, 3.5, gap=2.2),
            card(0, 0, w, h, 3.5, play_d(w / 2, h / 2, 11), gap=2.2)]


def fork():
    w = 4.4
    cw, ch = 20, 15
    top, split, bend, bottom, spread = ch + 2, ch + 7, ch + 17, ch + 26, 14
    items = [card(-cw / 2, 0, cw, ch, 3, play_d(0, ch / 2, 8)),
             line([(0, top), (0, bottom)], w)]
    for sgn in (-1, 1):
        curve = [(sgn * spread * (1 - math.cos(t * math.pi / 2)), split + (bend - split) * math.sin(t * math.pi / 2))
                 for t in [i / 20 for i in range(21)]]
        items.append(line(curve + [(sgn * spread, bottom)], w))
    return items


def trays():
    w = 4
    items = []
    for i in range(3):
        x = i * 14
        items.append(line([(x, 30), (x, 38), (x + 10, 38), (x + 10, 30)], w))
    cw, ch = 13, 17
    items.append(card(28 - cw / 2 + 5, 3, cw, ch, 2.6, play_d(28 + 5, 3 + ch / 2, 7), angle=14,
                      pivot=(33, 11.5)))
    return items


VARIANTS = [
    ("a1-frame", "Кадр над стопкой", frame_to_pile),
    ("a2-strip", "Кадр из ленты", strip_pulled),
    ("b1-fan", "Веер", fan),
    ("b2-stack", "Сдвинутая колода", offset_stack),
    ("c1-fork", "Развилка", fork),
    ("c2-trays", "Три лотка", trays),
]


def variant_svg(cid, key, in_circle):
    fn = next(v[2] for v in VARIANTS if v[0] == key)
    items = fn()
    if in_circle:
        return f'<circle cx="32" cy="32" r="32" fill="{INK}"/>' + glyph_svg(items, 42, PAPER, INK)
    return glyph_svg(items, 58, INK, "#ffffff")


NUMBER = 1
TITLE = "Волна 1 · направления"
PROMPT = ("Монохром. Каждый концепт в двух видах: в круге — иконка приложения, без подложки — пустые экраны. "
          "Упрощённый знак для шапки окна сделаю в конце от выбранной формы. Отметь, что вести во вторую волну, — можно несколько.")
CHOSEN = ["B2 в круге", "B2 без подложки"]
GROUPS = [("Плёнка → стопки", ["a1-frame", "a2-strip"]),
          ("Колода", ["b1-fan", "b2-stack"]),
          ("Раздача", ["c1-fork", "c2-trays"])]
NOTES = {
    "a1-frame": "Кадр плёнки с перфорацией и «play», под ним стопка. Перфорация — фирменный мотив раскадровки, но на 24 px дырочки сливаются в рамку.",
    "a2-strip": "Лента из трёх кадров, средний выдернут вправо — «этот сюда». Сортировка читается жестом, но силуэт высокий и узкий.",
    "b1-fan": "Три карты веером, передняя с «play». Сразу колода, держится на 24 px. Минус — веер карт похож на карточные игры.",
    "b2-stack": "Колода со сдвигом, передняя карта с «play». Спокойнее веера, ближе к приложению, но похоже на «копировать» или галерею.",
    "c1-fork": "Карта с «play» наверху, путь делится на три. Сортировка как схема — на 24 px превращается в медузу или корень зуба, в круге — в робота. Держу для полноты, но слабый.",
    "c2-trays": "Три лотка, карта с «play» падает в правый. Самый буквальный, но на 24 px карта и лотки мельчат.",
}
SUMMARY = ("Моё мнение: сильнее всех B1 и A2. Веер — самая узнаваемая колода и цел на 24 px; лента с выдернутым кадром — "
           "единственный вариант, где видно само действие сортировки. A1 и C2 на мелком размере теряют детали, "
           "C1 уводит в посторонние образы.")

CARDS = [(title, [{"code": k.split("-")[0].upper(), "name": next(v[1] for v in VARIANTS if v[0] == k),
                   "note": NOTES[k], "tiles": [(k.split("-")[0].upper() + " в круге", "в круге", k, True),
                             (k.split("-")[0].upper() + " без подложки", "без подложки", k, False)]}
                  for k in keys]) for title, keys in GROUPS]
