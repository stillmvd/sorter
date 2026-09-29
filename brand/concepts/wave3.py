import itertools
import re

import wave2
from wave1 import card, play_d, rrect_d, glyph_svg, INK, PAPER

W, H, R, D, GAP = 24, 30, 3.5, 5.5, 2.2
HOLES = [3, 4, 5]
PLAYS = [7, 9, 11]
DECK = [2, 3]
BASE = (1, 1, 1)


def mark(holes_i, play_i, deck_i):
    n = HOLES[holes_i]
    m = 3
    step = (H - 2 * m) / n
    hh = step * .55
    hw = {3: 3.4, 4: 3, 5: 2.6}[n]
    holes = "".join(rrect_d(side, m + step * k + (step - hh) / 2, hw, hh, .8)
                    for side in (1.8, W - 1.8 - hw) for k in range(n))
    holes += play_d(W / 2, H / 2, PLAYS[play_i])
    backs = [card(D * i, -D * i, W, H, R, gap=GAP if i < DECK[deck_i] - 1 else 0)
             for i in range(DECK[deck_i] - 1, 0, -1)]
    return backs + [card(0, 0, W, H, R, holes, gap=GAP)]


def key_of(combo):
    return "m" + "".join(map(str, combo))


def combo_of(key):
    return tuple(int(c) for c in key[1:])


def variant_svg(cid, key, in_circle):
    if not re.fullmatch(r"m\d{3}", key):
        return wave2.variant_svg(cid, key, in_circle)
    items = mark(*combo_of(key))
    if in_circle:
        return f'<circle cx="32" cy="32" r="32" fill="{INK}"/>' + glyph_svg(items, 42, PAPER, INK)
    return glyph_svg(items, 58, INK, "#ffffff")


def plain(items):
    return glyph_svg(items, 58, "currentColor", "BG").replace('fill="BG"', 'style="fill:var(--tbg)"')


def preview():
    rows, symbols = [], []
    for combo in itertools.product(range(len(HOLES)), range(len(PLAYS)), range(len(DECK))):
        k = key_of(combo)
        items = mark(*combo)
        symbols.append(f'<symbol id="w3-{k}-c" viewBox="0 0 64 64"><circle cx="32" cy="32" r="32" fill="{INK}"/>'
                       f'{glyph_svg(items, 42, PAPER, INK)}</symbol>'
                       f'<symbol id="w3-{k}-p" viewBox="0 0 64 64">{plain(items)}</symbol>')

        def use(kind, size=None, cls=""):
            attrs = (f' width="{size}" height="{size}"' if size else "") + (f' class="{cls}"' if cls else "")
            return f'<svg viewBox="0 0 64 64"{attrs} aria-hidden="true"><use href="#w3-{k}-{kind}"/></svg>'

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
                    f'<div class="build-tile light">{use("p", cls="big")}<span class="sizes">{sizes("p")}</span></div>'
                    f'<div class="build-tile dark">{use("p", cls="big")}<span class="sizes">{sizes("p")}</span></div>'
                    f'</div><div class="s-scenes">{scenes}</div></div>')
    sprite = f'<svg width="0" height="0" style="position:absolute" aria-hidden="true">{"".join(symbols)}</svg>'
    return f'<div class="build"><h3>Сборка</h3>{sprite}{"".join(rows)}</div>'


CSS = """
.build-tile.light{--tbg:#fff}
.build-tile.dark{--tbg:#141416}
.s-scenes{display:grid;gap:10px;grid-template-columns:repeat(auto-fit,minmax(min(100%,420px),1fr))}
.s-scene{--tbg:#f4f4f6;border-radius:14px;overflow:hidden;display:grid;background:var(--tbg);color:#17171a;
  border:1px solid #dedee2;font-size:12px}
.s-scene.dark{--tbg:#141416;color:#ececef;border-color:#2e2e33}
.s-title{display:flex;align-items:center;gap:8px;padding:8px 10px;font-size:12px}
.s-title span:first-of-type{flex:1;min-width:0}
.s-dots{letter-spacing:.6em;opacity:.6}
.s-empty{display:grid;justify-items:center;gap:6px;padding:28px 16px 34px;text-align:center}
.s-empty svg{opacity:.9}
.s-empty b{font-size:15px;font-weight:500;margin-top:8px}
.s-empty span{opacity:.6}
.s-taskbar{display:flex;justify-content:center;gap:6px;padding:6px;background:#202024}
.s-taskbar i,.s-on{width:36px;height:36px;border-radius:6px;display:grid;place-items:center}
.s-taskbar i::before{content:"";width:22px;height:22px;border-radius:5px;background:#3a3a40}
.s-on{background:#2d2d33}
"""

NUMBER = 3
TITLE = "Волна 3 · перфорация и пропорции"
PROMPT = ("Доводка B2c по параметрам: в каждой карточке выбери одно значение, сверху в «Сборке» — что получается "
          "в круге, без подложки на светлом и тёмном, в шапке окна, на пустом экране и в панели задач.")
CHOSEN = ["дырочки 4", "play 9", "карт 3"]


def param_tiles(label, index, values, suffix):
    tiles = []
    for i, v in enumerate(values):
        combo = list(BASE)
        combo[index] = i
        tiles.append((f"{label} {v}", f"{v} {suffix}", key_of(combo), True, i == BASE[index]))
    return tiles


CARDS = [("Параметры", [
    {"code": "Дырочки", "name": "на каждой стороне", "radio": True,
     "note": "Меньше дырочек — крупнее каждая, дольше держатся на 16–24 px. Пять похожи на плёнку, три — на кнопки.",
     "tiles": param_tiles("дырочки", 0, HOLES, "на сторону")},
    {"code": "Play", "name": "размер треугольника", "radio": True,
     "note": "Крупный «play» сразу говорит «видео», мелкий оставляет место перфорации.",
     "tiles": param_tiles("play", 1, PLAYS, "единиц")},
    {"code": "Колода", "name": "сколько карт", "radio": True,
     "note": "Две карты проще на 16 px, три — однозначнее колода.",
     "tiles": param_tiles("карт", 2, DECK, "карты")},
])]
SUMMARY = ("Моё мнение: 4 дырочки, play 9, три карты. Пять дырочек на 16 px сливаются в полосу, три уже не плёнка. "
           "Play 11 упирается в перфорацию, 7 теряется на мелком размере.")
