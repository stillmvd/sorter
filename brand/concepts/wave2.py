import wave1
from wave1 import card, play_d, rrect_d, glyph_svg, INK, PAPER

W, H, R = 24, 30, 3.5


def front(x=0, y=0, holes=None, gap=2.2, angle=0, pivot=(0, 0)):
    return card(x, y, W, H, R, play_d(x + W / 2, y + H / 2, 11) if holes is None else holes, gap=gap,
                angle=angle, pivot=pivot)


def two_cards():
    d = 6.5
    return [card(d, -d, W, H, R), front()]


def straight_up():
    return [card(6, -10, W - 12, H, 2.6), card(3, -5, W - 6, H, 3, gap=2), front(gap=2)]


def outlined_back():
    d, t = 8, 2.2

    def ring(x, y):
        return card(x, y, W, H, R, rrect_d(x + t, y + t, W - 2 * t, H - 2 * t, R - t / 2), gap=2.2)

    return [ring(2 * d, -2 * d), ring(d, -d), front()]


def perforated():
    d = 5.5
    holes = "".join(rrect_d(side, 3.2 + k * 5.4, 2.6, 2.6, .8) for side in (1.8, W - 4.4) for k in range(5))
    return [card(2 * d, -2 * d, W, H, R), card(d, -d, W, H, R, gap=2.2),
            front(holes=holes + play_d(W / 2, H / 2, 9))]


def dealt():
    d = 5.5
    return [card(d, -d, W, H, R), card(0, 0, W, H, R, gap=2.2),
            front(x=-7, y=4, angle=-14, pivot=(W / 2 - 7, H + 4))]


VARIANTS = [
    ("b2-two", "Две карты", two_cards),
    ("b2-ring", "Задние контуром", outlined_back),
    ("b2-perf", "Перфорация", perforated),
    ("b2-dealt", "Карта сходит", dealt),
]


def variant_svg(cid, key, in_circle):
    entry = next((v for v in VARIANTS if v[0] == key), None)
    if entry is None:
        return wave1.variant_svg(cid, key, in_circle)
    items = entry[2]()
    if in_circle:
        return f'<circle cx="32" cy="32" r="32" fill="{INK}"/>' + glyph_svg(items, 42, PAPER, INK)
    return glyph_svg(items, 58, INK, "#ffffff")


NUMBER = 2
TITLE = "Волна 2 · композиция колоды"
PROMPT = ("Меняется только композиция: сколько карт, куда сдвинуты, что на передней. В ряду добавил 16 px — "
          "так знак без подложки живёт в шапке окна. Отметь, что оставить, — можно несколько.")
CHOSEN = ["B2c в круге", "B2c без подложки"]
NOTES = {
    "b2-stack": "Как в волне 1: три карты со сдвигом вправо-вверх.",
    "b2-two": "Две карты, сдвиг больше. Просторнее на 16 px, но ближе к значку «копировать».",
    "b2-up": "Задние карты уже и выглядывают сверху — стопка, а не галерея. Силуэт выше и собраннее.",
    "b2-ring": "Задние карты контуром, передняя залита. Легче и глубже на крупном, на 16 px контуры сливаются.",
    "b2-perf": "Передняя карта с перфорацией плёнки по краям — связь с раскадровкой в интерфейсе. На 24 px дырочки уходят в шум.",
    "b2-dealt": "Верхняя карта сходит влево с наклоном — видно действие: карту сняли с колоды и несут в стопку.",
}
CODES = {"b2-stack": "B2", "b2-two": "B2a", "b2-ring": "B2b", "b2-perf": "B2c", "b2-dealt": "B2d"}
CARDS = [("Колода", [{"code": c, "name": "Было" if c == "B2" else next(v[1] for v in VARIANTS if v[0] == k),
                      "note": NOTES[k],
                      "tiles": [(f"{c} в круге", "в круге", k, True), (f"{c} без подложки", "без подложки", k, False)]}
                     for k, c in CODES.items()])]
SUMMARY = ("Моё мнение: B2d «Карта сходит» — самый живой и единственный про сортировку, в круге сильнее всех; "
           "на 16 px наклон даёт лесенку, но силуэт цел. B2a чище всех на 16 px, но ближе к «копировать». "
           "Перфорацию и контуры отбросил бы — их съедает мелкий размер. Стопку с узкими задними картами "
           "сняла сама проверка: на всех размерах это батарейка.")
