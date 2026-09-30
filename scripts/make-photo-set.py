import json
import os
import random
import shutil
from datetime import datetime, timedelta

from PIL import Image, ImageEnhance, ImageOps

SRC = r"G:\vk photos"
DST = r"G:\sorter-test\photos"
random.seed(4)


def exif(when=None, orientation=None, model="Redmi Note 5", make="Xiaomi"):
    e = Image.Exif()
    e[0x010F] = make
    e[0x0110] = model
    if orientation:
        e[0x0112] = orientation
    if when:
        e[0x0132] = when.strftime("%Y:%m:%d %H:%M:%S")
        e.get_ifd(0x8769)[0x9003] = when.strftime("%Y:%m:%d %H:%M:%S")
    return e


def base(name):
    im = ImageOps.exif_transpose(Image.open(os.path.join(SRC, name))).convert("RGB")
    im.thumbnail((1600, 1600))
    return im


def jitter(im, k):
    w, h = im.size
    dx, dy = int(w * random.uniform(0.01, 0.04)), int(h * random.uniform(0.01, 0.04))
    box = (dx * (k % 2), dy * ((k + 1) % 2), w - dx * ((k + 1) % 2), h - dy * (k % 2))
    out = im.crop(box).resize((w, h))
    return ImageEnhance.Brightness(out).enhance(random.uniform(0.92, 1.08))


def main():
    shutil.rmtree(DST, ignore_errors=True)
    os.makedirs(DST)
    names = sorted(n for n in os.listdir(SRC) if n.lower().endswith(".jpg"))
    picks = random.sample(names, 12)
    t = datetime(2024, 7, 14, 12, 0, 0)
    manifest = {"series": [], "singles": [], "dupes": [], "rotated": None}

    for i, size in enumerate([5, 3, 6]):
        im = base(picks[i])
        members = []
        for k in range(size):
            t += timedelta(seconds=random.randint(1, 3))
            name = f"IMG_series{i + 1}_{k + 1}.jpg"
            jitter(im, k).save(os.path.join(DST, name), quality=90, exif=exif(t))
            members.append(name)
        manifest["series"].append(members)
        t += timedelta(minutes=5)

    for j in range(6):
        t += timedelta(seconds=random.randint(1, 3))
        name = f"IMG_single_{j + 1}.jpg"
        base(picks[3 + j]).save(os.path.join(DST, name), quality=90, exif=exif(t))
        manifest["singles"].append(name)

    t += timedelta(hours=1)
    orig = base(picks[9])
    orig.save(os.path.join(DST, "dupe_orig.jpg"), quality=92, exif=exif(t))
    w, h = orig.size
    orig.resize((w // 2, h // 2)).save(os.path.join(DST, "dupe_half.jpg"), quality=90)
    orig.save(os.path.join(DST, "dupe_q60.jpg"), quality=60)
    orig.crop((int(w * 0.1), int(h * 0.1), int(w * 0.9), int(h * 0.9))).save(os.path.join(DST, "dupe_crop80.jpg"), quality=90)
    manifest["dupes"] = ["dupe_orig.jpg", "dupe_half.jpg", "dupe_q60.jpg", "dupe_crop80.jpg"]

    rot = base(picks[10])
    rot.rotate(90, expand=True).save(os.path.join(DST, "rotated_exif6.jpg"), quality=90, exif=exif(t + timedelta(days=1), orientation=6))
    manifest["rotated"] = {"file": "rotated_exif6.jpg", "width": rot.size[0], "height": rot.size[1]}

    base(picks[11]).save(os.path.join(DST, "no_exif.jpg"), quality=88)

    png = Image.new("RGBA", (800, 600), (0, 0, 0, 0))
    png.paste(base(picks[0]).resize((600, 400)), (100, 100))
    png.save(os.path.join(DST, "transparent.png"))

    webp = base(picks[1])
    webp.save(os.path.join(DST, "photo.webp"), quality=85)

    frames = [ImageEnhance.Brightness(base(picks[2]).resize((400, 300))).enhance(0.6 + 0.1 * k) for k in range(8)]
    frames[0].save(os.path.join(DST, "animated.gif"), save_all=True, append_images=frames[1:], duration=120, loop=0)
    frames[0].save(os.path.join(DST, "still.gif"))

    with open(os.path.join(DST, "broken.jpg"), "wb") as f:
        f.write(os.urandom(4096))

    with open(os.path.join(DST, "manifest.json"), "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)
    print(f"{len(os.listdir(DST)) - 1} файлов в {DST}")


if __name__ == "__main__":
    main()
