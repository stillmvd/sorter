import base64
import pathlib

import wave5
from wave5 import g

OUT = pathlib.Path(__file__).resolve().parent / "wave6"
OUT.mkdir(exist_ok=True)
SIZES = [48, 32, 24, 16]


def source(kind, bar):
    if kind == "now":
        return g.svg(g.small(bar))
    return g.svg(g.pixel_mark(kind == "twoc", bar), view="0 0 24 24")


def png(kind, bar, px):
    path = OUT / f"{kind}-{bar}-{px}.png"
    if not path.exists():
        src = OUT / f"{kind}-{bar}.svg"
        src.write_text(source(kind, bar), encoding="utf-8")
        g.export(src, path, px)
    return base64.b64encode(path.read_bytes()).decode()


def variant_svg(cid, key, in_circle):
    if not key.startswith("p-"):
        return wave5.variant_svg(cid, key, in_circle)
    _, kind, bar = key.split("-")
    suffix = cid.split("-")[-1]
    px = int(suffix) if suffix.isdigit() else 24
    return (f'<image href="data:image/png;base64,{png(kind, bar, px)}" width="64" height="64" '
            f'style="image-rendering:pixelated"/>')


def surface(key):
    return f" bar-{key.rsplit('-', 1)[1]}" if key.startswith("p-") else wave5.surface(key)


NUMBER = 6
TITLE = "Волна 6 · знак под 24 px"
PROMPT = ("Панель задач показывает иконку ровно 24 px, без сжатия. Крупно на плитке — те самые 24 пикселя, увеличенные; "
          "ниже — настоящие размеры. Отметь один.")
CHOSEN = ["B — две карты без круга", "без дырочек, play крупнее и скруглён"]
KINDS = [("now", "Как сейчас", "Три карты, обводка и зазоры по 1 px — на 24 px всё слипается, светлый круг сливается с картами."),
         ("two", "Две карты", "Нарисовано по пиксельной сетке 24: зазор и дырочки по 2 px, обводка 1 px. Без круга — "
                              "карты на всё поле."),
         ("twoc", "Две карты в круге", "То же на круге по теме: на тёмной панели круг светлый, выглядывает по углам.")]
CARDS = [("Панель задач", [
    {"code": chr(65 + i), "name": name, "note": note,
     "tiles": [(f"{chr(65 + i)} тёмная панель", "тёмная панель", f"p-{kind}-dark", False),
               (f"{chr(65 + i)} светлая панель", "светлая панель", f"p-{kind}-light", False)]}
    for i, (kind, name, note) in enumerate(KINDS)])]
SUMMARY = ("Моё мнение: B — две карты по сетке. Каждый элемент не меньше 2 px, поэтому на 24 px видно колоду, перфорацию и "
           "«play». Круг на 24 px отъедает место и снова превращается в пятно по углам. От 64 px остаётся полный знак.")
