import { Copy } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { isPhotoPath, media, placeLabel, type DupeGroup, type GroupItem } from "../../lib/ipc";
import { plural } from "../../lib/plural";
import { Button } from "../ui/Button";

type Mark = "keep" | "trash" | "not";

const clock = (ms: number) => {
  const s = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
};
const name = (path: string) => path.split("\\").pop() ?? path;
const mb = (b: number) => `${(b / 1024 ** 2).toFixed(1).replace(".", ",")} МБ`;
const date = (ms: number) => new Date(ms).toLocaleDateString("ru-RU", { day: "numeric", month: "long", year: "numeric" });
const pixels = (i: GroupItem) => (i.width ?? 0) * (i.height ?? 0);
const rate = (i: GroupItem) => (i.durationMs ? (i.size * 8 * 1000) / i.durationMs : 0);

const PILL: Record<Mark, string> = {
  keep: "bg-fg text-ink",
  trash: "border border-[rgb(236_236_239/25%)] bg-[rgb(20_20_22/72%)] text-[#ececef]",
  not: "border border-dashed border-[rgb(236_236_239/45%)] bg-[rgb(20_20_22/72%)] text-[#ececef]",
};

function action(keep: number, trash: number, not: number) {
  if (trash === 0) return not ? `Убрать из группы: ${not}` : "Оставить все";
  const rest = trash === 1 ? "одну в Корзину" : `остальные ${trash} в Корзину`;
  return `Оставить ${keep}, ${rest}`;
}

export function GroupCompare({
  group,
  title,
  onDone,
  onClose,
}: {
  group: DupeGroup;
  title: string;
  onDone: (keep: GroupItem[], trash: GroupItem[], not: GroupItem[]) => void;
  onClose: () => void;
}) {
  const items = group.items;
  const [marks, setMarks] = useState<Mark[]>(() => items.map((i) => (i.best ? "keep" : "trash")));
  const [focus, setFocus] = useState(0);
  const cards = useRef<(HTMLDivElement | null)[]>([]);
  const row = useRef<HTMLDivElement | null>(null);
  const [scroll, setScroll] = useState({ left: 0, width: 1 });

  const pick = (m: Mark) => items.filter((_, i) => marks[i] === m);
  const keep = pick("keep");
  const trash = pick("trash");
  const not = pick("not");
  const ready = keep.length > 0 || trash.length === 0;
  const finish = () => {
    if (ready && (trash.length || not.length)) onDone(keep, trash, not);
    else if (ready) onClose();
  };
  const toggle = (i: number, to: Mark) =>
    setMarks((ms) => ms.map((m, j) => (j !== i ? m : to === "not" ? (m === "not" ? "trash" : "not") : m === "keep" ? "trash" : "keep")));

  const keys = useRef<(e: KeyboardEvent) => void>(() => {});
  keys.current = (e) => {
    e.stopImmediatePropagation();
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      setFocus((f) => Math.max(0, Math.min(items.length - 1, f + (e.key === "ArrowRight" ? 1 : -1))));
    } else if (e.code === "Space") {
      e.preventDefault();
      toggle(focus, "keep");
    } else if (e.code === "KeyN") {
      e.preventDefault();
      toggle(focus, "not");
    } else if (e.key === "Enter") {
      e.preventDefault();
      finish();
    } else if (e.key === "Escape") {
      e.preventDefault();
      onClose();
    }
  };
  useEffect(() => {
    const listen = (e: KeyboardEvent) => keys.current(e);
    window.addEventListener("keydown", listen, true);
    return () => window.removeEventListener("keydown", listen, true);
  }, []);

  useEffect(() => {
    cards.current[focus]?.scrollIntoView({ inline: "nearest", block: "nearest", behavior: "smooth" });
  }, [focus]);

  const measure = () => {
    const el = row.current;
    if (el) setScroll({ left: el.scrollLeft / Math.max(el.scrollWidth, 1), width: el.clientWidth / Math.max(el.scrollWidth, 1) });
  };
  useEffect(measure, [items.length]);

  const photo = isPhotoPath(items[0]?.path ?? "");
  const most = (f: (i: GroupItem) => number) => {
    const vals = items.map(f);
    const top = Math.max(...vals);
    return (i: GroupItem) => top > 0 && f(i) === top && vals.some((v) => v !== top);
  };
  const bestPixels = most(pixels);
  const bestSize = most((i) => i.size);
  const bestRate = most(rate);
  const bestLength = most((i) => i.durationMs ?? 0);
  const rows = (i: GroupItem) =>
    [
      { k: "Разрешение", v: i.width && i.height ? `${i.width}×${i.height}` : "—", best: bestPixels(i) },
      { k: "Битрейт", v: rate(i) ? `${(rate(i) / 1e6).toFixed(1).replace(".", ",")} Мбит/с` : "—", best: bestRate(i) },
      { k: "Длина", v: i.durationMs ? clock(i.durationMs) : "—", best: bestLength(i) },
      { k: "Размер", v: mb(i.size), best: photo && bestSize(i) },
      { k: "Снято", v: i.takenAt ? date(i.takenAt) : "—", best: false },
    ].filter((r) => !photo || (r.k !== "Битрейт" && r.k !== "Длина"));

  const n = items.length;
  return (
    <div className="absolute inset-0 z-30 flex flex-col gap-[22px] overflow-hidden rounded-[28px] bg-cosmic py-7 pl-10">
      <div className="flex items-end gap-3 pr-10">
        <div className="flex flex-1 flex-col gap-3.5">
          <div className="inline-flex h-7 items-center gap-2 self-start rounded-full bg-raised px-3 text-xs font-medium text-dim">
            <Copy className="h-3.5 w-3.5" strokeWidth={1.5} />
            {title} · {group.confidence}% · {n} {plural(n, "копия", "копии", "копий")}
          </div>
          <h1 className="m-0 text-[42px] leading-[1.06] tracking-[-0.02em]">
            <span className="font-light">Оставить</span>{" "}
            <span className="font-bold">
              {keep.length} из {n}
            </span>
          </h1>
        </div>
        <Button hotkey="Esc" onClick={onClose}>
          Закрыть
        </Button>
      </div>

      <div className="relative min-h-0 flex-1">
        <div
          ref={row}
          onScroll={measure}
          onWheel={(e) => {
            if (row.current && Math.abs(e.deltaY) > Math.abs(e.deltaX)) row.current.scrollLeft += e.deltaY;
          }}
          className="flex h-full gap-7 overflow-x-auto overflow-y-hidden pt-1.5 pr-36 pl-1.5 [scrollbar-width:none]"
        >
          {items.map((it, i) => {
            const m = marks[i];
            return (
              <div
                key={it.path}
                ref={(el) => {
                  cards.current[i] = el;
                }}
                className={`flex w-[400px] shrink-0 flex-col gap-3.5 transition-opacity duration-200 ${m === "not" ? "opacity-45" : ""}`}
              >
                <button
                  type="button"
                  aria-label={`${name(it.path)} — ${m === "keep" ? "оставить" : m === "not" ? "не копия" : "в Корзину"}`}
                  onClick={() => (focus === i ? toggle(i, "keep") : setFocus(i))}
                  className={`relative h-[300px] shrink-0 overflow-hidden rounded-3xl bg-[#0c0c0e] ${focus === i ? "outline-[1.5px] outline-offset-4 outline-fg outline-solid" : ""}`}
                >
                  {isPhotoPath(it.path) ? (
                    <img src={media(it.path)} alt="" draggable={false} className="absolute inset-0 h-full w-full object-contain" />
                  ) : (
                    <video src={media(it.path)} autoPlay loop muted playsInline preload="auto" className="absolute inset-0 h-full w-full object-contain" />
                  )}
                  <span className={`absolute top-3 right-3 inline-flex h-7 items-center gap-1.5 rounded-full px-3 text-xs font-bold ${PILL[m]}`}>
                    {m === "keep" && (
                      <svg width="14" height="16" viewBox="0 0 26 30" fill="none" stroke="currentColor" strokeWidth="3" strokeLinecap="round">
                        <path d="M3 16c3 3 5 6 7 10C13 16 17 8 23 3" />
                      </svg>
                    )}
                    {m === "keep" ? "Оставить" : m === "not" ? "Не копия · уйдёт из группы" : "В Корзину"}
                  </span>
                </button>
                <div className="flex flex-col gap-0.5">
                  <div className="truncate text-base font-bold" title={it.path}>
                    {name(it.path)}
                  </div>
                  <div className={`truncate text-[13px] ${it.where === "folder" ? "text-fg" : "text-dim"}`} title={placeLabel(it)}>
                    {placeLabel(it)}
                  </div>
                </div>
                <div className="flex flex-col">
                  {rows(it).map((r) => (
                    <div key={r.k} className="flex justify-between border-b border-line py-[7px] text-[13px]">
                      <span className="text-dim">{r.k}</span>
                      <span className={r.best ? "font-bold" : "font-medium"}>{r.v}</span>
                    </div>
                  ))}
                </div>
              </div>
            );
          })}
        </div>
        {scroll.left + scroll.width < 0.999 && (
          <div className="pointer-events-none absolute top-0 right-0 bottom-0 w-36 bg-gradient-to-r from-transparent to-cosmic" />
        )}
      </div>

      <div className="flex flex-col gap-[18px] pr-10">
        {scroll.width < 0.999 && (
          <div className="h-1 rounded-full bg-raised">
            <div className="h-1 rounded-full bg-strong" style={{ marginLeft: `${scroll.left * 100}%`, width: `${scroll.width * 100}%` }} />
          </div>
        )}
        <div className="flex items-center gap-3">
          <div className="text-[13px] whitespace-nowrap text-dim">← → — копия · Пробел — оставить или в Корзину · колесо — листать</div>
          <div className="flex-1" />
          <Button variant="ghost" size={44} hotkey="N" onClick={() => toggle(focus, "not")} className="text-dim">
            Это не копия
          </Button>
          <Button variant="primary" size={44} hotkey="Enter" disabled={!ready} onClick={finish}>
            {action(keep.length, trash.length, not.length)}
          </Button>
        </div>
      </div>
    </div>
  );
}
