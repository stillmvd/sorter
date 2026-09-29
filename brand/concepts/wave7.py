import wave6
from wave6 import g, OUT, SIZES
import base64

PLAYS = [8, 9.5, 11]


def png(h, bar, px):
    path = OUT / f"play{h}-{bar}-{px}.png"
    if not path.exists():
        src = OUT / f"play{h}-{bar}.svg"
        src.write_text(g.svg(g.pixel_mark(False, bar, h), view="0 0 24 24"), encoding="utf-8")
        g.export(src, path, px)
    return base64.b64encode(path.read_bytes()).decode()


def variant_svg(cid, key, in_circle):
    if not key.startswith("q-"):
        return wave6.variant_svg(cid, key, in_circle)
    _, i, bar = key.split("-")
    suffix = cid.split("-")[-1]
    px = int(suffix) if suffix.isdigit() else 24
    return (f'<image href="data:image/png;base64,{png(PLAYS[int(i)], bar, px)}" width="64" height="64" '
            f'style="image-rendering:pixelated"/>')


def surface(key):
    return f" bar-{key.rsplit('-', 1)[1]}" if key.startswith("q-") else wave6.surface(key)


NUMBER = 7
TITLE = "Волна 7 · две карты, крупный play"
PROMPT = "Дырочки убраны, «play» крупнее и со скруглёнными углами. Три размера «play», крупно — пиксели 24 px. Отметь один."
CHOSEN = ["B — play 9,5 px"]
NAMES = ["Play 8 px", "Play 9,5 px", "Play 11 px"]
NOTES = ["Треугольник с запасом от краёв карты — спокойно, но на 16 px мелковат.",
         "Середина: крупный, до краёв карты около 2 px.",
         "Почти во всю карту — самый заметный, но у краёв тесно, на 16 px край карты и треугольник сливаются."]
CARDS = [("Панель задач", [
    {"code": chr(65 + i), "name": NAMES[i], "note": NOTES[i],
     "tiles": [(f"{chr(65 + i)} тёмная панель", "тёмная панель", f"q-{i}-dark", False),
               (f"{chr(65 + i)} светлая панель", "светлая панель", f"q-{i}-light", False)]}
    for i in range(3)])]
SUMMARY = "Моё мнение: B, 9,5 px — крупный и ещё не прилипает к краям карты."
