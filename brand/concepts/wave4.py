import itertools
import re

import wave3
from wave1 import card, play_d, glyph_svg, INK, PAPER

FRACTIONS = [66, 72, 78, 84]
CIRCLES = [("тёмный", INK, PAPER), ("светлый", PAPER, INK)]
SMALL = [("как есть", 1), ("3 дырочки", 0), ("без дырочек", None)]
BASE = (2, 0, 2)


def full():
    return wave3.mark(1, 1, 1)


def small(si):
    holes_i = SMALL[si][1]
    if holes_i is not None:
        return wave3.mark(holes_i, 1, 1)
    items = wave3.mark(1, 1, 1)
    items[-1] = card(0, 0, wave3.W, wave3.H, wave3.R, play_d(wave3.W / 2, wave3.H / 2, 11), gap=wave3.GAP)
    return items


def circle_svg(items, fi, ci):
    _, ground, ink = CIRCLES[ci]
    size = 64 * FRACTIONS[fi] / 100
    return f'<circle cx="32" cy="32" r="32" fill="{ground}"/>' + glyph_svg(items, size, ink, ground)


def variant_svg(cid, key, in_circle):
    if not re.fullmatch(r"f\d{3}s?", key):
        return wave3.variant_svg(cid, key, in_circle)
    fi, ci, si = (int(c) for c in key[1:4])
    items = small(si) if key.endswith("s") else full()
    return circle_svg(items, fi, ci)


def preview():
    rows, symbols = [], []
    for combo in itertools.product(range(len(FRACTIONS)), range(len(CIRCLES)), range(len(SMALL))):
        k = "".join(map(str, combo))
        fi, ci, si = combo
        symbols.append(f'<symbol id="w4-{k}-c" viewBox="0 0 64 64">{circle_svg(full(), fi, ci)}</symbol>'
                       f'<symbol id="w4-{k}-cs" viewBox="0 0 64 64">{circle_svg(small(si), fi, ci)}</symbol>'
                       f'<symbol id="w4-{k}-p" viewBox="0 0 64 64">{wave3.plain(full())}</symbol>'
                       f'<symbol id="w4-{k}-ps" viewBox="0 0 64 64">{wave3.plain(small(si))}</symbol>')

        def use(kind, size=None, cls=""):
            ref = kind + ("s" if size and size <= 24 else "")
            attrs = (f' width="{size}" height="{size}"' if size else "") + (f' class="{cls}"' if cls else "")
            return f'<svg viewBox="0 0 64 64"{attrs} aria-hidden="true"><use href="#w4-{k}-{ref}"/></svg>'

        def sizes(kind):
            return "".join(f'<span class="size">{use(kind, px)}{px}</span>' for px in (48, 32, 24, 16))

        scenes = "".join(
            f'<div class="s-scene {theme}"><div class="s-title">{use("p", 16)}<span>Sorter — G:\\vk videos</span>'
            f'<span class="s-dots">— ▢ ✕</span></div><div class="s-empty">{use("p", 72)}'
            f'<b>Колода пуста</b><span>Все 187 видео разложены по стопкам</span></div>'
            f'<div class="s-taskbar"><i></i><i></i><span class="s-on">{use("c", 24)}</span><i></i></div></div>'
            for theme in ("light", "dark"))
        hidden = "" if combo == BASE else " hidden"
        rows.append(f'<div class="build" data-combo="{"-".join(map(str, combo))}"{hidden}><div class="build-row">'
                    f'<div class="build-tile light">{use("c", cls="big")}<span class="sizes">{sizes("c")}</span></div>'
                    f'<div class="build-tile dark">{use("c", cls="big")}<span class="sizes">{sizes("c")}</span></div>'
                    f'<div class="build-tile light">{use("p", cls="big")}<span class="sizes">{sizes("p")}</span></div>'
                    f'<div class="build-tile dark">{use("p", cls="big")}<span class="sizes">{sizes("p")}</span></div>'
                    f'</div><div class="s-scenes">{scenes}</div></div>')
    sprite = f'<svg width="0" height="0" style="position:absolute" aria-hidden="true">{"".join(symbols)}</svg>'
    return f'<div class="build"><h3>Сборка</h3>{sprite}{"".join(rows)}</div>'


def tiles(index, labels, prefix):
    out = []
    for i, label in enumerate(labels):
        combo = list(BASE)
        combo[index] = i
        out.append((f"{prefix} {label}", label, "f" + "".join(map(str, combo)) + ("s" if index == 2 else ""), True,
                    i == BASE[index]))
    return out


NUMBER = 4
TITLE = "Волна 4 · финал формы"
PROMPT = ("Форма собрана: 4 дырочки, play 9, три карты. Осталось: доля глифа в круге, цвет круга и упрощение "
          "для 24 px и меньше — в «Сборке» мелкие размеры и шапка окна берут упрощённый вариант.")
CHOSEN = ["доля 66 %", "круг тёмный", "мелко как есть"]
CARDS = [("Параметры", [
    {"code": "Доля", "name": "глифа в круге", "radio": True,
     "note": "Рабочий диапазон 72–80 %: меньше — знак тонет в круге, больше — карты липнут к краю, "
             "и Windows подрезает крайние пиксели на мелких размерах.",
     "tiles": tiles(0, [f"{f} %" for f in FRACTIONS], "доля")},
    {"code": "Круг", "name": "цвет подложки", "radio": True,
     "note": "Тёмный — как главное действие в интерфейсе (инверсия), на тёмной панели задач держится обводкой "
             "светлых карт. Светлый ярче на тёмной панели, но на светлом рабочем столе растворяется.",
     "tiles": tiles(1, [c[0] for c in CIRCLES], "круг")},
    {"code": "Мелко", "name": "24 px и меньше", "radio": True,
     "note": "Как выглядит знак на 24 и 16 px: те же 4 дырочки, крупнее 3 или карта только с «play».",
     "tiles": tiles(2, [s[0] for s in SMALL], "мелко")},
])]
SUMMARY = ("Моё мнение: 78 %, тёмный круг, на мелких размерах — без дырочек. На 16 px перфорация всё равно "
           "превращается в серую кашу, а колода с «play» остаётся читаемой; дырочки живут от 32 px и выше.")
