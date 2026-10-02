import os
import sys
from pathlib import Path

SOURCES = [Path(r"G:\vk videos"), Path(r"G:\vk photos")]
MEDIA = {".mp4", ".mov", ".mkv", ".webm", ".avi", ".m4v", ".jpg", ".jpeg", ".png", ".webp", ".gif"}


def main():
    count = int(sys.argv[1]) if len(sys.argv) > 1 else 5000
    root = Path(sys.argv[2]) if len(sys.argv) > 2 else Path(r"G:\sorter-test")
    deck = root / ("big" if count >= 1000 else f"small-{count}")
    table = root / "big-table"
    deck.mkdir(parents=True, exist_ok=True)
    for pile in ("Мемы", "Друзья", "Поездки"):
        (table / pile).mkdir(parents=True, exist_ok=True)
    files = sorted(
        (f for src in SOURCES for f in src.iterdir() if f.is_file() and f.suffix.lower() in MEDIA),
        key=lambda f: (f.suffix.lower() in {".mp4", ".mov", ".mkv", ".webm", ".avi", ".m4v"}, f.name),
    )
    files = [f for pair in zip(files[: len(files) // 2], files[len(files) // 2 :]) for f in pair] + files[len(files) // 2 * 2 :]
    made = 0
    for i in range(count):
        src = files[i % len(files)]
        k = i // len(files)
        name = src.name if k == 0 else f"{src.stem}_{k}{src.suffix}"
        dst = deck / name
        if not dst.exists():
            os.link(src, dst)
            made += 1
    print(f"{deck}: {count} файлов ({made} новых ссылок), стол {table}")


if __name__ == "__main__":
    main()
