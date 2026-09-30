import { Copy } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { isPhotoPath, media, type Card, type DupeView } from "../../lib/ipc";
import { Button } from "../ui/Button";

type Side = {
  mine: boolean;
  path: string;
  where: string;
  width: number | null;
  height: number | null;
  durationMs: number | null;
  size: number;
  takenAt: number | null;
  start: number;
};

const clock = (ms: number) => {
  const s = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};
const span = (from: number, to: number) => `${clock(from)}⁠–⁠${clock(to)}`;
const name = (path: string) => path.split("\\").pop() ?? path;
const mb = (b: number) => `${(b / 1024 ** 2).toFixed(1).replace(".", ",")} МБ`;
const date = (ms: number) => new Date(ms).toLocaleDateString("ru-RU", { day: "numeric", month: "long", year: "numeric" });
const pixels = (s: Side) => (s.width ?? 0) * (s.height ?? 0);
const bitrate = (s: Side) => (s.durationMs ? (s.size * 8 * 1000) / s.durationMs : 0);

function roles(dupe: DupeView, sides: Side[]): string[] {
  if (dupe.kind === "trim") return sides.map((s) => ((s.durationMs ?? 0) >= Math.max(...sides.map((x) => x.durationMs ?? 0)) ? "оригинал" : "обрезка"));
  if (dupe.kind === "crop") return sides.map((s) => (pixels(s) >= Math.max(...sides.map(pixels)) ? "полный кадр" : "кадрирована"));
  if (dupe.kind === "same") return sides.map((_, i) => (i === 0 ? "лучше качество" : "хуже качество"));
  return ["", ""];
}

function signals(dupe: DupeView) {
  if (dupe.kind === "exact") return "файлы совпали побайтно";
  const hit = [
    (dupe.audio ?? 0) >= 0.9 && "звук",
    (dupe.visual ?? 0) >= 0.6 && "кадры",
    (dupe.semantic ?? 0) >= 0.85 && "смысл",
  ].filter(Boolean);
  return hit.length ? `${hit.join(", ").replace(/, ([^,]+)$/, " и $1")} совпали` : "похожи";
}

export function Compare({
  card,
  dupe,
  volume,
  onKeep,
  onDismiss,
  onClose,
}: {
  card: Card;
  dupe: DupeView;
  volume: number;
  onKeep: (mine: boolean) => void;
  onDismiss: () => void;
  onClose: () => void;
}) {
  const current: Side = {
    mine: true,
    path: card.path,
    where: "в колоде · эта карта",
    width: card.width,
    height: card.height,
    durationMs: card.durationMs,
    size: card.size,
    takenAt: card.takenAt,
    start: 0,
  };
  const other: Side = {
    mine: false,
    path: dupe.path,
    where: dupe.where === "pile" ? `в стопке «${dupe.pileName}»` : "в колоде",
    width: dupe.width,
    height: dupe.height,
    durationMs: dupe.durationMs,
    size: dupe.size,
    takenAt: dupe.takenAt,
    start: dupe.offsetMs ?? 0,
  };
  const sides = dupe.better ? [other, current] : [current, other];
  const role = roles(dupe, sides);
  const [picked, setPicked] = useState(0);
  const videos = useRef<(HTMLVideoElement | null)[]>([]);

  const keep = sides[picked];
  const label = (i: number) =>
    dupe.kind === "trim" ? (role[i] === "оригинал" ? "оригинал" : "обрезку") : sides[i].mine ? "эту карту" : "копию";
  const action =
    keep.mine && dupe.where === "pile"
      ? `Заменить копию в «${dupe.pileName}»`
      : `Оставить ${label(picked)}, ${label(1 - picked)} в Корзину`;

  const keys = useRef<(e: KeyboardEvent) => void>(() => {});
  keys.current = (e) => {
    e.stopImmediatePropagation();
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      setPicked(e.key === "ArrowLeft" ? 0 : 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      onKeep(keep.mine);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    } else if (e.code === "KeyN") {
      e.preventDefault();
      onDismiss();
    }
  };
  useEffect(() => {
    const listen = (e: KeyboardEvent) => keys.current(e);
    window.addEventListener("keydown", listen, true);
    return () => window.removeEventListener("keydown", listen, true);
  }, []);

  const master = sides[0].start >= sides[1].start ? 0 : 1;
  useEffect(() => {
    const id = window.setInterval(() => {
      const m = videos.current[master];
      const s = videos.current[1 - master];
      if (!m || !s || m.paused) return;
      const target = m.currentTime + (sides[master].start - sides[1 - master].start) / 1000;
      if (Math.abs(s.currentTime - target) > 0.25) s.currentTime = target;
      if (s.paused) void s.play().catch(() => {});
    }, 200);
    return () => window.clearInterval(id);
  }, [master, sides[0].start, sides[1].start]);

  useEffect(() => {
    videos.current.forEach((v, i) => {
      if (!v) return;
      v.muted = i !== picked || volume === 0;
      v.volume = volume;
    });
  }, [picked, volume]);

  const longest = Math.max(...sides.map((s) => s.durationMs ?? 0));
  const origin = Math.min(...sides.map((s) => s.start));
  const trimBar = (s: Side) => {
    if (dupe.kind !== "trim" || !longest) return null;
    const isOriginal = (s.durationMs ?? 0) === longest;
    const short = sides.find((x) => x !== s) ?? s;
    const left = isOriginal ? ((short.start - origin) / longest) * 100 : 0;
    const width = isOriginal ? ((short.durationMs ?? 0) / longest) * 100 : 100;
    return { left, width };
  };

  const heading =
    dupe.kind === "trim"
      ? (() => {
          const short = sides.reduce((a, b) => ((a.durationMs ?? 0) <= (b.durationMs ?? 0) ? a : b));
          const from = short.start - origin;
          return ["Обрезка:", `${span(from, from + (short.durationMs ?? 0))} из ${clock(longest)}`];
        })()
      : dupe.kind === "same"
        ? isPhotoPath(dupe.path)
          ? ["Та же", "фотография"]
          : ["Та же запись,", "другое качество"]
        : dupe.kind === "crop"
          ? ["Другое", "кадрирование"]
          : ["Точная", "копия"];

  const photo = isPhotoPath(dupe.path);
  const rows = (s: Side, o: Side) =>
    [
      { k: "Разрешение", v: s.width && s.height ? `${s.width}×${s.height}` : "—", best: pixels(s) > pixels(o) },
      { k: "Битрейт", v: bitrate(s) ? `${(bitrate(s) / 1e6).toFixed(1).replace(".", ",")} Мбит/с` : "—", best: bitrate(s) > bitrate(o) * 1.05 },
      { k: "Длина", v: s.durationMs ? clock(s.durationMs) : "—", best: (s.durationMs ?? 0) > (o.durationMs ?? 0) + 1000 },
      { k: "Размер", v: mb(s.size), best: photo && s.size > o.size * 1.05 },
      { k: "Снято", v: s.takenAt ? date(s.takenAt) : "—", best: false },
    ].filter((r) => !photo || (r.k !== "Битрейт" && r.k !== "Длина"));

  return (
    <div className="absolute inset-0 z-30 flex flex-col gap-[22px] rounded-[28px] bg-cosmic px-10 py-7">
      <div className="flex items-end gap-3">
        <div className="flex flex-1 flex-col gap-3.5">
          <div className="inline-flex h-7 items-center gap-2 self-start rounded-full bg-raised px-3 text-xs font-medium text-dim">
            <Copy className="h-3.5 w-3.5" strokeWidth={1.5} />
            Дубль · {dupe.confidence}% · {signals(dupe)}
          </div>
          <h1 className="m-0 text-[42px] leading-[1.06] tracking-[-0.02em]">
            <span className="font-light">{heading[0]}</span> <span className="font-bold">{heading[1]}</span>
          </h1>
        </div>
        <Button hotkey="Esc" onClick={onClose}>
          Закрыть
        </Button>
      </div>

      <div className="flex min-h-0 flex-1 gap-10">
        {sides.map((s, i) => {
          const bar = trimBar(s);
          return (
            <div key={s.path} className="contents">
              {i === 1 && <div className="w-px shrink-0 bg-line" />}
              <div className="flex min-w-0 flex-1 flex-col gap-3.5">
                <button
                  type="button"
                  aria-pressed={picked === i}
                  aria-label={`Выбрать ${name(s.path)}`}
                  onClick={() => setPicked(i)}
                  className={`relative min-h-[200px] flex-1 overflow-hidden rounded-3xl bg-[#0c0c0e] ${picked === i ? "outline-[1.5px] outline-offset-4 outline-fg outline-solid" : ""}`}
                >
                  {photo ? (
                    <img src={media(s.path)} alt="" draggable={false} className="absolute inset-0 h-full w-full object-contain" />
                  ) : (
                    <video
                      ref={(el) => {
                        videos.current[i] = el;
                      }}
                      src={media(s.path)}
                      autoPlay
                      loop={i === master}
                      muted
                      playsInline
                      preload="auto"
                      className="absolute inset-0 h-full w-full object-contain"
                    />
                  )}
                  {picked === i && (
                    <span className="absolute top-3 right-3 inline-flex h-7 items-center gap-1.5 rounded-full bg-fg px-3 text-xs font-bold text-ink">
                      <svg width="14" height="16" viewBox="0 0 26 30" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round">
                        <path d="M3 16c3 3 5 6 7 10C13 16 17 8 23 3" />
                      </svg>
                      Оставить
                    </span>
                  )}
                  {bar && (
                    <span className="absolute right-3.5 bottom-3.5 left-3.5 h-1 rounded-full bg-[rgb(236_236_239/20%)]">
                      <span className="block h-1 rounded-full bg-[#ececef]" style={{ marginLeft: `${bar.left}%`, width: `${bar.width}%` }} />
                    </span>
                  )}
                </button>
                <div className="flex flex-col gap-0.5">
                  <div className="truncate text-base font-bold" title={s.path}>
                    {name(s.path)}
                  </div>
                  <div className="text-[13px] text-dim">
                    {s.where}
                    {role[i] && ` · ${role[i]}`}
                  </div>
                </div>
                <div className="flex flex-col">
                  {rows(s, sides[1 - i]).map((r) => (
                    <div key={r.k} className="flex justify-between border-b border-line py-[7px] text-[13px]">
                      <span className="text-dim">{r.k}</span>
                      <span className={r.best ? "font-bold" : "font-medium"}>{r.v}</span>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          );
        })}
      </div>

      <div className="flex items-center gap-3">
        <div className="text-[13px] text-dim">Играют синхронно · ← → — выбрать, какую оставить</div>
        <div className="flex-1" />
        <Button variant="ghost" size={44} hotkey="N" onClick={onDismiss} className="text-dim">
          {isPhotoPath(dupe.path) ? "Это разные фото" : "Это разные видео"}
        </Button>
        <Button variant="primary" size={44} hotkey="Enter" onClick={() => onKeep(keep.mine)}>
          {action}
        </Button>
      </div>
    </div>
  );
}
